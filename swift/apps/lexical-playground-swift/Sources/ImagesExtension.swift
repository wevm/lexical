import Foundation
import Lexical
import LexicalExtension
import LexicalHTML
import LexicalRichText
import LexicalUtils

// Port of packages/lexical-playground/src/plugins/ImagesExtension and
// DragDropPasteExtension: `INSERT_IMAGE_COMMAND`, the `<img>`, `<figure>`
// and `<figcaption>` import rules, a click that selects an image (the
// `CLICK_COMMAND` handler of ImageComponent), and images dropped or pasted
// as files. Dragging an image to move it (the extension's DRAGSTART and
// DROP handlers) is not ported yet.

/// `isGoogleDocCheckboxImg`.
private func isGoogleDocCheckboxImg(_ img: DOMElement) -> Bool {
  img.parentElement?.tagName == "LI" && img.previousSibling == nil
    && img.getAttribute("aria-roledescription") == "checkbox"
}

/// The image an `<img>` imports to, or `nil` for one the rule leaves to
/// the next.
func importImage(_ el: DOMElement) throws -> ImageNode? {
  guard let src = el.getAttribute("src"), !src.isEmpty, !src.hasPrefix("file:///"),
    !isGoogleDocCheckboxImg(el)
  else { return nil }
  // `el.width` and `el.height`: the attributes as numbers, 0 when absent.
  return try createImageNode(
    ImagePayload(
      altText: el.getAttribute("alt") ?? "", height: el.getAttribute("height").flatMap(Double.init),
      src: src, width: el.getAttribute("width").flatMap(Double.init)))
}

nonisolated(unsafe) private let ImgRule = defineImportRule(
  name: "@lexical/playground/img", match: sel.tag("img")
) { _, el, next in
  if let image = try importImage(el) { return [image] }
  return try next()
}

nonisolated(unsafe) private let FigcaptionRule = defineImportRule(
  name: "@lexical/playground/figcaption", match: sel.tag("figcaption")
) { _, _, _ in [] }

nonisolated(unsafe) private let FigureRule = defineImportRule(
  name: "@lexical/playground/figure", match: sel.tag("figure")
) { ctx, el, _ in
  let imported = try ctx.importChildren(el)
  if let figcaption = el.getElementsByTagName("figcaption").first {
    for case let image as ImageNode in imported {
      try image.importCaption(figcaption)
    }
  }
  return imported
}

/// `InsertImagePayload`.
typealias InsertImagePayload = ImagePayload

let INSERT_IMAGE_COMMAND = LexicalCommand<InsertImagePayload>("INSERT_IMAGE_COMMAND")

/// `$toggleNodeSelection` for an image's click: a Shift click adds it to
/// the node selection or takes it out, a plain click selects only it.
private func selectImage(_ node: ImageNode, _ shiftKey: Bool) throws {
  let selection = try getSelection()
  if shiftKey, let selection = selection as? NodeSelection {
    if selection.has(node.getKey()) {
      selection.delete(node.getKey())
    } else {
      selection.add(node.getKey())
    }
    return
  }
  let nodeSelection = createNodeSelection()
  nodeSelection.add(node.getKey())
  try setSelection(nodeSelection)
}

/// `ImagesExtension`.
let ImagesExtension = defineExtension(
  name: "@lexical/playground/Images",
  dependencies: [
    configExtension(DOMImportExtension) {
      $0.rules = [FigcaptionRule, FigureRule, ImgRule].map(\.entry)
    }
  ],
  nodes: { [.klass(ImageNode.self)] },
  register: { editor, _, _ in
    mergeRegister([
      editor.registerCommand(
        INSERT_IMAGE_COMMAND,
        { payload, _ in
          let imageNode = try createImageNode(payload)
          try insertNodes([imageNode])
          if try isRootOrShadowRoot(imageNode.getParentOrThrow()) {
            _ = try wrapNodeInElement(imageNode) { try createParagraphNode() }.selectEnd()
          }
          return true
        }, COMMAND_PRIORITY_EDITOR),
      // ImageComponent's `CLICK_COMMAND` handler: the views report a click
      // on a decorator with its key.
      editor.registerCommand(
        CLICK_COMMAND,
        { event, _ in
          guard let key = event.targetKey, let node = try getNodeByKey(key) as? ImageNode
          else { return false }
          try selectImage(node, event.shiftKey)
          return true
        }, COMMAND_PRIORITY_LOW),
    ])
  })

private let ACCEPTABLE_IMAGE_TYPES = [
  "image/", "image/heic", "image/heif", "image/gif", "image/webp",
]

/// `DragDropPasteExtension`: image files dropped or pasted into the editor
/// (`DRAG_DROP_PASTE`) become images, each with its file name as its alt
/// text and a `data:` URL as its source, as `mediaFileReader` reads them.
let DragDropPasteExtension = defineExtension(
  name: "@lexical/playground/DragDropPaste",
  register: { editor, _, _ in
    editor.registerCommand(
      DRAG_DROP_PASTE,
      { files, _ in
        let images = files.filter { isMimeType($0, ACCEPTABLE_IMAGE_TYPES) }
        if images.isEmpty { return false }
        // JS reads the files asynchronously, then inserts each.
        let payloads = images.map {
          InsertImagePayload(
            altText: $0.name,
            src: "data:\($0.type);base64,\(Data($0.data).base64EncodedString())")
        }
        nonisolated(unsafe) let editor = editor
        DispatchQueue.main.async {
          for payload in payloads {
            _ = try? editor.dispatchCommand(INSERT_IMAGE_COMMAND, payload)
          }
        }
        return true
      }, COMMAND_PRIORITY_LOW)
  })
