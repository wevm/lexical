import Foundation
import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHTML
import LexicalLink
import LexicalRichText
import LexicalUtils

// Port of packages/lexical-playground/src/plugins/ImagesExtension and
// DragDropPasteExtension: `INSERT_IMAGE_COMMAND`, the `<img>`, `<figure>`
// and `<figcaption>` import rules, a click that selects an image (the
// `CLICK_COMMAND` handler of ImageComponent), and images dropped or pasted
// as files, and dragging a selected image to move it (the extension's
// DRAGSTART, DRAGOVER and DROP handlers).

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
      editor.registerCommand(
        DRAGSTART_COMMAND, { event, _ in try onDragStart(event) }, COMMAND_PRIORITY_HIGH),
      editor.registerCommand(
        DRAGOVER_COMMAND, { event, editor in try onDragover(event, editor) },
        COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        DROP_COMMAND, { event, editor in try onDrop(event, editor) }, COMMAND_PRIORITY_HIGH),
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

/// `$getImageNodeInSelection`.
private func getImageNodeInSelection() throws -> ImageNode? {
  guard let selection = try getSelection() as? NodeSelection else { return nil }
  return try selection.getNodes().first as? ImageNode
}

/// `$onDragStart`: a selected image drags its payload as
/// `application/x-lexical-drag` (the views drag it without an image, as JS
/// sets a transparent drag image).
private func onDragStart(_ event: DragEvent) throws -> Bool {
  guard let node = try getImageNodeInSelection(), let dataTransfer = event.dataTransfer else {
    return false
  }
  func dimension(_ value: ImageDimension) -> Any {
    if case .points(let points) = value { return points }
    return "inherit"
  }
  // The caption is its editor, which the drop takes from the node, as JS
  // serializes the editor object.
  let drag: [String: Any] = [
    "data": [
      "altText": node.__altText, "height": dimension(node.__height), "key": node.getKey(),
      "maxWidth": node.__maxWidth, "showCaption": node.__showCaption, "src": node.__src,
      "width": dimension(node.__width),
    ],
    "type": "image",
  ]
  dataTransfer.setData("text/plain", "_")
  dataTransfer.setData(
    "application/x-lexical-drag",
    String(decoding: try JSONSerialization.data(withJSONObject: drag), as: UTF8.self))
  return true
}

/// `$onDragover`.
private func onDragover(_ event: DragEvent, _ editor: Editor) throws -> Bool {
  guard try getImageNodeInSelection() != nil else { return false }
  if try dropCaret(event, editor) == nil { event.preventDefault() }
  return false
}

/// `$onDrop`: the dragged image leaves its place and is inserted again at
/// the drop point, inside the link it was in.
private func onDrop(_ event: DragEvent, _ editor: Editor) throws -> Bool {
  guard let node = try getImageNodeInSelection(), var data = getDragImageData(event) else {
    return false
  }
  let existingLink =
    try findMatchingParent(node) { $0 is LinkNode && !($0 is AutoLinkNode) } as? LinkNode
  event.preventDefault()
  if let caret = try dropCaret(event, editor) {
    data.caption = node.caption
    try node.remove()
    try setSelection(setSelectionFromCaretRange(getCollapsedCaretRange(caret)))
    _ = try editor.dispatchCommand(INSERT_IMAGE_COMMAND, data)
    if let existingLink {
      _ = try editor.dispatchCommand(TOGGLE_LINK_COMMAND, .url(try existingLink.getURL()))
    }
  }
  return true
}

/// `getDragImageData`.
private func getDragImageData(_ event: DragEvent) -> InsertImagePayload? {
  guard let json = event.dataTransfer?.getData("application/x-lexical-drag"), !json.isEmpty,
    let object = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any],
    object["type"] as? String == "image", let data = object["data"] as? [String: Any],
    let src = data["src"] as? String
  else { return nil }
  return InsertImagePayload(
    altText: data["altText"] as? String ?? "", height: data["height"] as? Double,
    key: data["key"] as? String, maxWidth: data["maxWidth"] as? Double ?? 500,
    showCaption: data["showCaption"] as? Bool, src: src, width: data["width"] as? Double)
}

/// `canDropImage` and `getDragSelection`: the caret at the drop point,
/// unless it is in code or on an image.
func dropCaret(_ event: DragEvent, _ editor: Editor) throws -> PointCaret? {
  guard
    let caret = try (editor.getRootElement() as? LexicalRootElement)?.caretFromPoint(
      event.clientX, event.clientY),
    !(caret.origin is ImageNode),
    try findMatchingParent(caret.origin, { $0 is CodeNode }) == nil
  else { return nil }
  return caret
}

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
