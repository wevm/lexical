import Lexical
import LexicalExtension
import LexicalHTML

// Port of packages/lexical-playground/src/PlaygroundDOMRenderExtension.ts:
// a paragraph whose exported children include a block (an image's
// `<figure>`) exports as `<div role="paragraph">`, since a `<p>` cannot
// hold one.

nonisolated(unsafe) private let ParagraphExportOverride = domOverride(
  [ParagraphNode.self],
  DOMRenderMatchConfig(exportDOM: { _, next, _ in
    var output = try next()
    guard let element = output.element as? DOMElement, element.tagName == "P" else {
      return output
    }
    let innerAfter = output.after
    output.after = { generated in
      var generated = generated
      if let innerAfter { generated = try innerAfter(generated) }
      guard let paragraph = generated as? DOMElement, paragraph.tagName == "P" else {
        return generated
      }
      guard paragraph.childNodes.contains(where: isBlockDomNode) else { return paragraph }
      let div = getDocument().createElement("div")
      div.setAttribute("role", "paragraph")
      for attribute in paragraph.attributes { div.setAttribute(attribute.name, attribute.value) }
      while let child = paragraph.firstChild { _ = div.appendChild(child) }
      return div
    }
    return output
  }))

/// `PlaygroundDOMRenderExtension`.
let PlaygroundDOMRenderExtension = defineExtension(
  name: "@lexical/playground/DOMRender",
  dependencies: [
    configExtension(DOMRenderExtension) { $0.overrides = [ParagraphExportOverride] }
  ])
