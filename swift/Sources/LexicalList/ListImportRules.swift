import Lexical
import LexicalHTML

// Port of packages/lexical-list/src/ListImportExtension.ts: the
// `DOMImportExtension` rules for lists (`@experimental` upstream), which
// `ListExtension` contributes.

/// Mirrors the legacy `isDomChecklist` heuristic.
private func isChecklistElement(_ el: DOMElement) -> Bool {
  el.getAttribute("__lexicallisttype") == "check" || el.classList.contains("contains-task-list")
    || el.getAttribute("data-is-checklist") == "1"
    || el.children.contains { $0.hasAttribute("aria-checked") }
}

/// Lifts nested `ListNode`s out of `ListItemNode`s into sibling
/// `ListItemNode`s (the legacy `$normalizeChildren` shape), and wraps any
/// other child in a new item. The wrapper items come from
/// `listNode.createListItemNode()`, so a `ListNode` subclass gets its own
/// item type here too.
private func normalizeListChildren(
  _ children: [LexicalNode], _ listNode: ListNode
) throws
  -> [ListItemNode]
{
  var out: [ListItemNode] = []
  for child in children {
    if let item = child as? ListItemNode {
      out.append(item)
      let innerChildren = try item.getChildren()
      if innerChildren.count > 1 {
        for inner in innerChildren where inner is ListNode {
          out.append(try listNode.createListItemNode().append(inner))
        }
      }
    } else {
      out.append(try listNode.createListItemNode().append(child))
    }
  }
  return out
}

nonisolated(unsafe) private let ListRule = defineImportRule(
  name: "@lexical/list/list", match: sel.tag("ol", "ul")
) { ctx, el, _ in
  let node: ListNode
  if isElementOfTag(el, "ol") {
    node = try createListNode("number", start: olStart(el))
  } else if isChecklistElement(el) {
    node = try createListNode("check")
  } else {
    node = try createListNode("bullet")
  }
  try setDirectionFromDOM(node, el)
  // The list's `text-align` goes onto each item, where the reconciler
  // renders it (legacy `wrapContinuousInlines` did the same).
  return [
    try node.splice(
      0, 0,
      propagateTextAlignToBlockChildren(
        normalizeListChildren(ctx.importChildren(el), node), el))
  ]
}

/// Applies the format of a sole paragraph child of `<li>` to the item and
/// unwraps it (Google Docs aligns the `<p>` inside the `<li>`). Mirrors the
/// legacy `setFormatFromChildren`.
private func liftFormatFromSingleParagraph(
  _ listItemNode: ListItemNode, _ children: [LexicalNode]
) throws -> [LexicalNode] {
  guard children.count == 1, let paragraph = children[0] as? ParagraphNode else {
    return children
  }
  if try listItemNode.getFormatType().isEmpty, try !paragraph.getFormatType().isEmpty {
    try listItemNode.setFormat(paragraph.getFormatType())
    return try paragraph.getChildren()
  }
  return children
}

/// Collapses block children of a `<li>` into inline-with-line-break form:
/// contiguous inline siblings stay together, and one line break goes
/// between runs (`<li>1<div>2</div>3</li>` → `1<br>2<br>3`). Any block
/// (`isBlockLevel`) is a boundary except a nested `ListNode`, which
/// `normalizeListChildren` lifts into a sibling. Where the editor enforces
/// node structure, a childless block decorator splits the list instead, so
/// no line break goes next to it.
private func flattenListItemBlocks(_ children: [LexicalNode]) throws -> [LexicalNode] {
  func isBoundary(_ node: LexicalNode) throws -> Bool {
    try isBlockLevel(node) && !(node is ListNode)
  }
  guard try children.contains(where: isBoundary) else { return children }
  var segments: [(nodes: [LexicalNode], lifted: Bool)] = []
  var inlineRun: [LexicalNode] = []
  func flushInlineRun() {
    if !inlineRun.isEmpty {
      segments.append((inlineRun, false))
      inlineRun = []
    }
  }
  let lifts = try INTERNAL_isNodeStructureEnforced(getEditor())
  for child in children {
    if try isBoundary(child) {
      flushInlineRun()
      if let element = child as? ElementNode {
        segments.append((try element.getChildren(), false))
      } else {
        segments.append(([child], lifts))
      }
    } else {
      inlineRun.append(child)
    }
  }
  flushInlineRun()
  var out: [LexicalNode] = []
  for (i, segment) in segments.enumerated() {
    if !out.isEmpty, !segments[i - 1].lifted, !segment.lifted {
      out.append(try createLineBreakNode())
    }
    out += segment.nodes
  }
  return out
}

private func importListItem(
  _ ctx: DOMImportContext, _ el: DOMElement, checked: Bool?
) throws -> [LexicalNode] {
  let node = try createListItemNode(checked)
  try setFormatFromDOM(node, el)
  try setDirectionFromDOM(node, el)
  // The sole paragraph's format is lifted before flattening, which would
  // unwrap the paragraph and lose it.
  return [
    try node.splice(
      0, 0, flattenListItemBlocks(liftFormatFromSingleParagraph(node, ctx.importChildren(el))))
  ]
}

nonisolated(unsafe) private let ListItemRule = defineImportRule(
  name: "@lexical/list/li", match: sel.tag("li")
) { ctx, el, _ in
  let ariaChecked = el.getAttribute("aria-checked")
  return try importListItem(
    ctx, el, checked: ariaChecked == "true" ? true : ariaChecked == "false" ? false : nil)
}

/// A checklist item checked by `input` (`<input type="checkbox">`).
private func buildChecklistItem(
  _ ctx: DOMImportContext, _ el: DOMElement, _ input: DOMElement
)
  throws -> [LexicalNode]
{
  try importListItem(ctx, el, checked: input.hasAttribute("checked"))
}

private func checkboxChild(_ el: DOMElement) -> DOMElement? {
  el.children.first { isElementOfTag($0, "input") && $0.getAttribute("type") == "checkbox" }
}

nonisolated(unsafe) private let TaskListItemRule = defineImportRule(
  name: "@lexical/list/li-task-list-item", match: sel.tag("li").classAll("task-list-item")
) { ctx, el, next in
  guard let input = checkboxChild(el) else { return try next() }
  return try buildChecklistItem(ctx, el, input)
}

nonisolated(unsafe) private let JoplinChecklistItemRule = defineImportRule(
  name: "@lexical/list/li-joplin-checkbox", match: sel.tag("li").classAll("joplin-checkbox")
) { ctx, el, next in
  guard let wrapper = el.children.first(where: { $0.classList.contains("checkbox-wrapper") }),
    let input = checkboxChild(wrapper)
  else { return try next() }
  return try buildChecklistItem(ctx, el, input)
}

/// `ListSchema` (`@experimental`): a `ChildSchema` that accepts only
/// `ListItemNode` and (immediately nested) `ListNode` children; runs of
/// other children are wrapped in a new `ListItemNode`.
nonisolated(unsafe) public let ListSchema = ChildSchema(
  name: "ListSchema", accepts: { child, _ in child is ListItemNode || child is ListNode },
  packageRun: { run, _, _ in [try createListItemNode().splice(0, 0, run)] })

/// `ListImportRules` (`@experimental`): the import rules for `ListNode`
/// and `ListItemNode`, with the GitHub task list and Joplin checkbox
/// heuristics. `ListExtension` contributes them, with `CoreImportExtension`.
/// The class-restricted rules come first so they win over the generic `li`.
nonisolated(unsafe) public let ListImportRules = [
  TaskListItemRule, JoplinChecklistItemRule, ListRule, ListItemRule,
]
