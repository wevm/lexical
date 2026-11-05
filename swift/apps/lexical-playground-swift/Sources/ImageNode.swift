import Foundation
import Lexical
import LexicalExtension
import LexicalHTML
import LexicalLink
import LexicalRichText
import LexicalUtils

// Port of packages/lexical-playground/src/nodes/ImageNode.tsx: an image
// with its alt text, size, maximum width and a caption, which is a nested
// editor, serialized as its own editor state. `decorate` draws it with
// `ImageComponent` (ImageComponent.swift).

/// An image's width or height: `'inherit'` (the image's own) or points.
enum ImageDimension: Equatable {
  case inherit
  case points(Double)

  /// `width || 'inherit'`: an absent or 0 size is unsized.
  init(_ value: Double?) {
    if let value, value != 0, !value.isNaN { self = .points(value) } else { self = .inherit }
  }

  /// `'inherit'` serializes as 0.
  var serialized: Double {
    if case .points(let value) = self { return value }
    return 0
  }

  var points: Double? {
    if case .points(let value) = self { return value }
    return nil
  }

  /// `toString()`: `inherit`, or the number as JS writes it.
  var attributeValue: String {
    switch self {
    case .inherit: "inherit"
    case .points(let value):
      value.rounded() == value && abs(value) < 1e15 ? String(Int64(value)) : String(value)
    }
  }
}

/// `CaptionEditorExtension`: the caption's nested editor, empty until it
/// is given a state. Upstream's caption also has hashtags, keywords, emoji
/// and mentions, which the Swift playground does not register there yet.
let CaptionEditorExtension = defineExtension(
  name: "@lexical/playground/ImageNodeCaption",
  dependencies: [RichTextExtension, LinkExtension],
  editorConfig: InitialEditorConfig(
    namespace: "Playground/ImageNodeCaption", initialEditorState: InitialEditorStateType.none))

/// `ImagePayload`.
struct ImagePayload {
  var altText: String
  var caption: Editor?
  var height: Double?
  var key: NodeKey?
  var maxWidth: Double = 500
  var showCaption: Bool?
  var src: String
  var width: Double?
  var captionsEnabled: Bool?
}

/// `$isCaptionEditorEmpty`: whether the caption has no text, line break
/// or decorator.
func isCaptionEditorEmpty() throws -> Bool {
  try !dfs(getRoot()).contains { !($0.node is ElementNode) }
}

/// `ImageNode`.
final class ImageNode: DecoratorNode {
  var __src = ""
  var __altText = ""
  var __width = ImageDimension.inherit
  var __height = ImageDimension.inherit
  var __maxWidth: Double = 500
  var __showCaption = false
  /// Built when first read, so that clones, which share their original's,
  /// don't each build one.
  fileprivate var _caption: Editor?
  /// Captions cannot yet be used within editor cells.
  var __captionsEnabled = true

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      ImageNode.self, type: "image", extends: DecoratorNode.self,
      json: [
        SchemaField("altText", stringValue(), field: \ImageNode.__altText),
        SchemaField(
          "caption", rawValue(), of: ImageNode.self,
          get: { .object(try $0.getSerializedCaption()) },
          set: { try $0.setCaption($1?.objectValue) }),
        // An unsized dimension is the 'inherit' sentinel, which has always
        // serialized as 0 (and parses back through `|| 'inherit'`).
        SchemaField(
          "height", optional(numberValue()), of: ImageNode.self,
          get: { .number(try $0.getLatest().__height.serialized) },
          set: { try $0.setHeight($1.flatMap(\.numberValue)) }),
        // Read straight from the field, but applied through setMaxWidth: an
        // absent `maxWidth` keeps the constructor's default.
        SchemaField(
          "maxWidth", optional(numberValue()), of: ImageNode.self,
          get: { .number(try $0.getLatest().__maxWidth) },
          set: { try $0.setMaxWidth($1.flatMap(\.numberValue)) }),
        SchemaField("showCaption", booleanValue(), field: \ImageNode.__showCaption),
        SchemaField("src", stringValue(), field: \ImageNode.__src),
        SchemaField(
          "width", optional(numberValue()), of: ImageNode.self,
          get: { .number(try $0.getLatest().__width.serialized) },
          set: { try $0.setWidth($1.flatMap(\.numberValue)) }),
      ])
  }

  /// The Swift playground imports HTML with `generateNodesFromDOM`, which
  /// `ImagesExtension`'s rules do not reach, so the node converts `<img>`
  /// itself as well, as upstream's `importDOM` did before the rules
  /// replaced it.
  override class func importDOM() -> DOMConversionMap? {
    [
      "img": { node in
        guard let el = node as? DOMElement, (try? importImage(el)) != nil else { return nil }
        return DOMConversion { node in
          DOMConversionOutput(node: [try importImage(node as! DOMElement)].compactMap { $0 })
        }
      }
    ]
  }

  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    let prev = prevNode as! ImageNode
    __src = prev.__src
    __altText = prev.__altText
    __width = prev.__width
    __height = prev.__height
    __maxWidth = prev.__maxWidth
    __showCaption = prev.__showCaption
    _caption = prev.caption
    __captionsEnabled = prev.__captionsEnabled
  }

  /// `__caption`.
  var caption: Editor {
    if let _caption { return _caption }
    // The extension is the playground's own and always builds.
    let editor = try! buildEditorFromExtensions(CaptionEditorExtension)
    _caption = editor
    return editor
  }

  /// The nested caption editor's own serialized state.
  func getSerializedCaption() throws -> JSONObject { try getLatest().caption.toJSON() }

  /// Applies a serialized nested caption editor; an empty state is ignored
  /// so it does not clobber the caption the node was created with.
  @discardableResult
  func setCaption(_ caption: JSONObject?) throws -> Self {
    let writable = try getWritable()
    if let state = caption?["editorState"]?.objectValue {
      let nested = writable.caption
      let editorState = try nested.parseEditorState(state)
      if !editorState.isEmpty() {
        try nested.setEditorState(editorState)
      }
    }
    return writable
  }

  override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let document = getDocument()
    let img = document.createElement("img")
    img.setAttribute("src", __src)
    img.setAttribute("alt", __altText)
    img.setAttribute("width", __width.attributeValue)
    img.setAttribute("height", __height.attributeValue)
    if __showCaption {
      let captionEditor = caption
      let captionHtml = try captionEditor.read { () -> String? in
        if try isCaptionEditorEmpty() { return nil }
        // Don't serialize the wrapping paragraph if there is only one.
        var selection: RangeSelection?
        if let firstChild = try getRoot().getFirstChild() as? ParagraphNode,
          try firstChild.getNextSibling() == nil
        {
          let range = try createRangeSelection()
          try range.anchor.set(firstChild.getKey(), 0, .element)
          try range.focus.set(firstChild.getKey(), firstChild.getChildrenSize(), .element)
          selection = range
        }
        return try generateHtmlFromNodes(captionEditor, selection)
      }
      if let captionHtml, !captionHtml.isEmpty {
        let figure = document.createElement("figure")
        let figcaption = document.createElement("figcaption")
        // `figcaption.innerHTML = captionHtml`.
        let parsed = DOMParser().parseFromString(captionHtml)
        for child in parsed.body?.childNodes ?? [] { _ = figcaption.appendChild(child) }
        _ = figure.appendChild(img)
        _ = figure.appendChild(figcaption)
        return DOMExportOutput(element: figure)
      }
    }
    return DOMExportOutput(element: img)
  }

  @discardableResult
  func setSrc(_ src: String) throws -> Self {
    let writable = try getWritable()
    writable.__src = src
    return writable
  }

  @discardableResult
  func setAltText(_ altText: String) throws -> Self {
    let writable = try getWritable()
    writable.__altText = altText
    return writable
  }

  @discardableResult
  func setMaxWidth(_ maxWidth: Double?) throws -> Self {
    let writable = try getWritable()
    if let maxWidth { writable.__maxWidth = maxWidth }
    return writable
  }

  /// An unsized image serializes as 0 (and older documents may omit the
  /// property), both of which restore the 'inherit' sentinel.
  @discardableResult
  func setWidth(_ width: Double?) throws -> Self {
    let writable = try getWritable()
    writable.__width = ImageDimension(width)
    return writable
  }

  @discardableResult
  func setHeight(_ height: Double?) throws -> Self {
    let writable = try getWritable()
    writable.__height = ImageDimension(height)
    return writable
  }

  @discardableResult
  func setWidthAndHeight(_ width: ImageDimension, _ height: ImageDimension) throws -> Self {
    let writable = try getWritable()
    writable.__width = width
    writable.__height = height
    return writable
  }

  @discardableResult
  func setShowCaption(_ showCaption: Bool) throws -> Self {
    let writable = try getWritable()
    writable.__showCaption = showCaption
    return writable
  }

  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let span = getDocument().createElement("span")
    if let className = config.theme["image"]?.stringValue { span.className = className }
    return span
  }

  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }

  func getSrc() throws -> String { try getLatest().__src }

  func getAltText() throws -> String { try getLatest().__altText }

  override func decorate(_ editor: Editor) throws -> Any? {
    let (key, src, altText, width, height, maxWidth, showCaption) = (
      getKey(), __src, __altText, __width, __height, __maxWidth, __showCaption
    )
    let captionText =
      showCaption ? (try? caption.read { try getRoot().getTextContent() }) ?? "" : ""
    // The views decorate on the main thread.
    nonisolated(unsafe) let editor = editor
    return MainActor.assumeIsolated {
      ImageComponent(
        editor: editor, nodeKey: key, src: src, altText: altText, width: width, height: height,
        maxWidth: maxWidth, showCaption: showCaption, caption: captionText)
    }
  }
}

/// `$createImageNode(payload)`.
func createImageNode(_ payload: ImagePayload) throws -> ImageNode {
  let node = try ImageNode(key: payload.key)
  node.__src = payload.src
  node.__altText = payload.altText
  node.__maxWidth = payload.maxWidth
  node.__width = ImageDimension(payload.width)
  node.__height = ImageDimension(payload.height)
  node.__showCaption = payload.showCaption ?? false
  node._caption = payload.caption
  node.__captionsEnabled = payload.captionsEnabled != false
  return try applyNodeReplacement(node)
}

func isImageNode(_ node: LexicalNode?) -> Bool { node is ImageNode }
