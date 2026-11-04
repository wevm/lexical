import Lexical
import LexicalExtension
import LexicalHTML

// Port of packages/lexical-list/src/WordListImportExtension.ts.

nonisolated(unsafe) private let WORD_LIST_CLASS_RE = /^MsoListParagraph(CxSp(First|Middle|Last))?$/
nonisolated(unsafe) private let WORD_NUMBERED_RE = /^[A-Za-z0-9]+[.)]/
nonisolated(unsafe) private let WORD_GENERATOR_RE = /Microsoft Word/.ignoresCase()

/// `mso-list` is a Microsoft property that the stylesheet-inlining
/// preprocess drops when it rewrites an element's `style`, so the overlay's
/// preprocess, which runs first, copies it to this attribute.
private let MSO_LIST_DATA_ATTR = "data-mso-list"

private func readWordListLevel(_ el: DOMElement) -> Int {
  // mso-list looks like "l<N> level<M> lfo<X>"; pluck the level number.
  guard let match = (el.getAttribute(MSO_LIST_DATA_ATTR) ?? "").firstMatch(of: /level(\d+)/)
  else { return 1 }
  return Int(match.1) ?? 1
}

private func findMarkerSpan(_ el: DOMElement) -> DOMElement? {
  el.descendantElements().first {
    $0.localName == "span" && $0.getAttribute(MSO_LIST_DATA_ATTR) == "Ignore"
  }
}

private func readWordMarker(_ el: DOMElement) -> String {
  guard let span = findMarkerSpan(el) else { return "" }
  return jsTrim(span.textContent)
}

private func classifyWordListType(_ marker: String) -> ListType {
  marker.prefixMatch(of: WORD_NUMBERED_RE) != nil ? "number" : "bullet"
}

/// Removes the marker span with the `<span>`s that wrap it directly under
/// the `<p>`.
private func stripWordMarker(_ el: DOMElement) {
  guard let inner = findMarkerSpan(el) else { return }
  var outer: DOMElement = inner
  while let parent = outer.parentNode as? DOMElement, parent !== el, parent.nodeName == "SPAN" {
    outer = parent
  }
  outer.remove()
}

private func isWordListParagraph(_ node: DOMNode?) -> DOMElement? {
  guard let el = node as? DOMElement, el.className.wholeMatch(of: WORD_LIST_CLASS_RE) != nil
  else { return nil }
  return el
}

private struct WordListItem {
  var el: DOMElement
  var level: Int
  var marker: String
}

private func buildWordListTree(
  _ ctx: DOMImportContext, _ items: [WordListItem]
) throws
  -> ListNode
{
  let root = try createListNode(classifyWordListType(items[0].marker))
  var stack: [(list: ListNode, level: Int)] = [(root, items[0].level)]
  for item in items {
    while stack.count > 1, stack[stack.count - 1].level > item.level {
      stack.removeLast()
    }
    if item.level > stack[stack.count - 1].level {
      // A sublist lives in its own wrapper ListItemNode, a sibling of the
      // items above it (`isNestedListNode`).
      let sub = try createListNode(classifyWordListType(item.marker))
      try stack[stack.count - 1].list.append(createListItemNode().append(sub))
      stack.append((sub, item.level))
    }
    stripWordMarker(item.el)
    try stack[stack.count - 1].list.append(
      createListItemNode().splice(
        0, 0, ctx.importChildren(item.el, ImportChildrenOpts(schema: InlineSchema))))
  }
  return root
}

/// The `<p class="MsoListParagraph*">` elements an earlier sibling's run
/// already took, per import session; `nil` until the rule installs one.
private final class ConsumedElements {
  var elements: [ObjectIdentifier: DOMElement] = [:]
}

private let WordListConsumed = createImportState(
  "@lexical/list/word-consumed-list-items"
) { ConsumedElements?.none }

nonisolated(unsafe) private let WordListParagraphRule = defineImportRule(
  name: "@lexical/list/word-list-paragraph",
  match: sel.tag("p").classAny(
    "MsoListParagraph", "MsoListParagraphCxSpFirst", "MsoListParagraphCxSpMiddle",
    "MsoListParagraphCxSpLast")
) { ctx, el, _ in
  let consumed: ConsumedElements
  if let existing = ctx.session.get(WordListConsumed) {
    consumed = existing
  } else {
    consumed = ConsumedElements()
    ctx.session.set(WordListConsumed, consumed)
  }
  if consumed.elements[ObjectIdentifier(el)] != nil {
    return []
  }
  var items: [WordListItem] = []
  var cur: DOMNode? = el
  while let paragraph = isWordListParagraph(cur) {
    consumed.elements[ObjectIdentifier(paragraph)] = paragraph
    items.append(
      WordListItem(
        el: paragraph, level: readWordListLevel(paragraph), marker: readWordMarker(paragraph)))
    // MsoListParagraph (no CxSp suffix) is a single-item run.
    if paragraph.classList.contains("MsoListParagraphCxSpLast")
      || paragraph.className == "MsoListParagraph"
    {
      break
    }
    cur = paragraph.nextElementSibling
  }
  return [try buildWordListTree(ctx, items)]
}

/// `<o:p>` is Office's paragraph end marker, in every Word paragraph; it
/// imports as nothing.
nonisolated(unsafe) private let WordOfficeParagraphRule = defineImportRule(
  name: "@lexical/list/word-o-p", match: sel.tag("o:p")
) { _, _, _ in [] }

nonisolated(unsafe) private let WordListPasteOverlay = defineOverlayRules([
  WordOfficeParagraphRule, WordListParagraphRule,
])

/// Where the document has `<meta name="Generator" content="Microsoft
/// Word…">`, copies the `mso-list` declarations to `MSO_LIST_DATA_ATTR`
/// and pushes the Word overlay onto `ImportOverlays`. Pastes from other
/// sources pay only the detection.
private func installWordListPasteOverlay(
  _ dom: DOMNode, _ ctx: DOMPreprocessContext, _ next: () throws -> Void
) throws {
  let elements =
    (dom as? DOMElement)?.descendantElements()
    ?? dom.childNodes.flatMap {
      ($0 as? DOMElement).map { [$0] + $0.descendantElements() } ?? []
    }
  let meta = elements.first { $0.localName == "meta" && $0.getAttribute("name") == "Generator" }
  if let meta, (meta.getAttribute("content") ?? "").firstMatch(of: WORD_GENERATOR_RE) != nil {
    for el in elements {
      guard let style = el.getAttribute("style"), style.contains("mso-list") else { continue }
      if let msoList = getStyleObjectFromCSS(style)["mso-list"], !msoList.isEmpty {
        el.setAttribute(MSO_LIST_DATA_ATTR, msoList)
      }
    }
    ctx.session.update(ImportOverlays) { $0 + [WordListPasteOverlay] }
  }
  try next()
}

/// `WordListImportExtension` (`@experimental`): opt-in support for lists
/// pasted from MS Word, which writes a list as a flat run of
/// `<p class="MsoListParagraph*">` with the marker in a
/// `<span style="mso-list:Ignore">` and `style="mso-list:l<N> level<M>
/// lfo<X>"` naming the list and its depth. `ListExtension` does not depend
/// on it.
public let WordListImportExtension = defineExtension(
  name: "@lexical/list/WordListImport",
  dependencies: [
    // The overlay builds ListNodes, so the nodes have to be registered.
    ListExtension,
    configExtension(DOMImportExtension) { $0.preprocess = [installWordListPasteOverlay] },
  ])
