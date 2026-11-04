import Lexical
import LexicalExtension
import LexicalHTML
import LexicalSelection

// Port of packages/lexical-clipboard/src/clipboard.ts. Native platforms have
// no DOM clipboard: `copyToClipboard` fills a `ClipboardEvent`'s
// `DataTransfer` (or one standing for the pasteboard), and the drop
// handlers take the drop caret the view resolved, where JS hit-tests the
// DOM (DESIGN.md, Clipboard).

/// `LexicalClipboardData`: the serialized selection by MIME type
/// (`text/plain` always, `text/html` and `application/x-lexical-editor`
/// when available, and any custom type), in the order JS sets them.
public typealias LexicalClipboardData = MimeTypeMap<String>

/// `BaseSerializedNode`: a node of a clipboard payload, read without
/// knowing its type.
public typealias BaseSerializedNode = JSONObject

/// `$getHtmlContent(editor, selection)`: the HTML of `selection`, relying
/// on each node's `exportDOM`; `""` when nothing is selected.
public func getHtmlContent(_ editor: Editor, _ selection: (any BaseSelection)?) throws -> String {
  guard let selection else {
    throw LexicalError("Expected valid LexicalSelection")
  }
  // If we haven't selected anything
  if try (selection is RangeSelection && selection.isCollapsed()) || selection.getNodes().isEmpty {
    return ""
  }
  return try generateHtmlFromNodes(editor, selection)
}

/// `$getHtmlContent(editor)`: the HTML of the current selection.
public func getHtmlContent(_ editor: Editor) throws -> String {
  try getHtmlContent(editor, getSelection())
}

/// `$getLexicalContent(editor, selection)`: the Lexical JSON of
/// `selection` as a string, relying on each node's `exportJSON`; `nil`
/// when nothing is selected.
public func getLexicalContent(
  _ editor: Editor, _ selection: (any BaseSelection)?
) throws -> String? {
  guard let selection else {
    throw LexicalError("Expected valid LexicalSelection")
  }
  // If we haven't selected anything
  if try (selection is RangeSelection && selection.isCollapsed()) || selection.getNodes().isEmpty {
    return nil
  }
  return try generateJSONFromSelectedNodes(editor, selection).toJSON().stringify()
}

/// `$getLexicalContent(editor)`: the Lexical JSON of the current selection.
public func getLexicalContent(_ editor: Editor) throws -> String? {
  try getLexicalContent(editor, getSelection())
}

/// `$insertDataTransferForPlainText`: inserts `text/plain` (or, without
/// it, `text/uri-list`) at `selection`.
public func insertDataTransferForPlainText(
  _ dataTransfer: DataTransfer, _ selection: any BaseSelection
) throws {
  let plain = dataTransfer.getData("text/plain")
  let text = plain.isEmpty ? dataTransfer.getData("text/uri-list") : plain
  try selection.insertRawText(text)
}

/// `$insertDataTransferForRichText`: inserts the contents of
/// `dataTransfer` at `selection` with the rich-text import pipeline
/// (`application/x-lexical-editor`, `text/html`, `text/plain`,
/// `text/uri-list`, by default in that order). Every payload type leaves
/// the editor's selection after the inserted content, so `selection` must
/// be a live selection this update may write to. `editor` is unused, as in
/// JS.
public func insertDataTransferForRichText(
  _ dataTransfer: DataTransfer, _ selection: any BaseSelection, _ editor: Editor? = nil
) throws {
  _ = try getImportOutput().insertDataTransfer(dataTransfer, selection)
}

private let LEXICAL_DRAG_MIME_TYPE = "application/x-lexical-drag"

/// `$writeDragSourceToDataTransfer`: marks `dataTransfer` as a drag that
/// started in `editor`, so the drop side can remove the source range.
public func writeDragSourceToDataTransfer(_ dataTransfer: DataTransfer, _ editor: Editor) {
  let marker = JSONObject([("editorKey", .string(editor.getKey()))])
  dataTransfer.setData(LEXICAL_DRAG_MIME_TYPE, marker.stringify())
}

/// `readDragMarker`: the source editor's key, or `nil` when the drag did
/// not start in a Lexical editor.
private func readDragMarker(_ dataTransfer: DataTransfer) -> String? {
  let raw = dataTransfer.getData(LEXICAL_DRAG_MIME_TYPE)
  if raw.isEmpty {
    return nil
  }
  guard case .object(let parsed)? = try? JSONValue.parse(raw) else {
    return nil
  }
  return parsed["editorKey"]?.stringValue
}

// A text edge and the element point beside it are the same drop position.
// Inline wrappers do not introduce another position at their outer edges,
// but block boundaries, line breaks, and decorators do.
private func normalizeDropBoundary(_ point: PointCaret) throws -> PointCaret {
  var caret = point
  if caret.isText {
    if caret.offset == 0 {
      caret = try rewindSiblingCaret(caret.getSiblingCaret())
    } else if try caret.offset == caret.origin.getTextContentSize() {
      caret = caret.getSiblingCaret()
    } else {
      return caret
    }
  }
  while true {
    guard let parent = try caret.getParentAtCaret(), try parent.isInline(), !parent.isShadowRoot()
    else {
      return caret
    }
    if caret.isChild {
      caret = try rewindSiblingCaret(getSiblingCaret(parent, .next))
    } else if try caret.getNodeAtCaret() == nil {
      caret = getSiblingCaret(parent, .next)
    } else {
      return caret
    }
  }
}

private func isDropCaretInsideSelection(
  _ dropCaret: PointCaret, _ selection: RangeSelection
) throws -> Bool {
  let range = try getCaretRangeInDirection(caretRangeFromSelection(selection), .next)
  let drop = try normalizeDropBoundary(dropCaret)
  return try comparePointCaretNext(normalizeDropBoundary(range.anchor), drop) <= 0
    && comparePointCaretNext(drop, normalizeDropBoundary(range.focus)) <= 0
}

private func doDrop(
  _ event: DragEvent, _ editor: Editor, _ dropCaret: PointCaret?,
  _ insertDataTransfer: (DataTransfer, any BaseSelection, Editor) throws -> Void
) throws -> Bool {
  guard let dataTransfer = event.dataTransfer else {
    return false
  }
  // Drags that didn't originate in a Lexical editor (no marker) fall
  // through to the platform's own drop handling.
  guard let sourceEditorKey = readDragMarker(dataTransfer) else {
    return false
  }
  guard let dropCaret else {
    return false
  }
  let isSameEditorDrag = sourceEditorKey == editor.getKey()
  let currentSelection = try getSelection()

  if isSameEditorDrag {
    // Same-editor drag: remove the dragged range ourselves, then insert at
    // the stable drop caret.
    guard let range = currentSelection as? RangeSelection, !range.isCollapsed() else {
      return false
    }
    if try isDropCaretInsideSelection(dropCaret, range) {
      event.preventDefault()
      return true
    }
  }

  // Prefer a text boundary to its containing element: deletion can merge
  // blocks and remove the element while its unselected children survive.
  // Only split text; splitting an element here would add a paragraph break.
  let normalizedDropCaret = try normalizeCaret(dropCaret)
  let stableDropCaret =
    try normalizedDropCaret.isText
    ? splitAtPointCaretNext(normalizedDropCaret) : normalizedDropCaret
  guard let stableDropCaret else {
    return false
  }
  let oppositeDropCaret = try stableDropCaret.getFlipped()

  if isSameEditorDrag, let range = currentSelection as? RangeSelection {
    try range.removeText()
  }

  // Either side can move with the surviving content. Keep both rather than
  // falling back to the source selection when one origin is removed.
  let insertCaret = try stableDropCaret.origin.isAttached() ? stableDropCaret : oppositeDropCaret
  try invariant(
    insertCaret.origin.isAttached(), "$doDrop: drop position was removed by source deletion")
  let dropSelection = try setSelectionFromCaretRange(getCollapsedCaretRange(insertCaret))
  try insertDataTransfer(dataTransfer, dropSelection, editor)

  // A cross-editor drag leaves the source range to the source editor's
  // view, which ends its drag session as a move (DESIGN.md, Clipboard).
  event.preventDefault()
  return true
}

/// `$handleRichTextDrop`: inserts the drag payload with
/// `insertDataTransferForRichText` at `dropCaret` and, for a drag from this
/// editor (marked by `writeDragSourceToDataTransfer`), removes the source
/// range. `dropCaret` is the caret the view resolved at the drop point,
/// where JS calls `caretFromPoint`; `nil` declines the drop.
public func handleRichTextDrop(
  _ event: DragEvent, _ editor: Editor, _ dropCaret: PointCaret?
) throws -> Bool {
  try doDrop(event, editor, dropCaret) { try insertDataTransferForRichText($0, $1, $2) }
}

/// `$handlePlainTextDrop`: `handleRichTextDrop`, inserting with
/// `insertDataTransferForPlainText`.
public func handlePlainTextDrop(
  _ event: DragEvent, _ editor: Editor, _ dropCaret: PointCaret?
) throws -> Bool {
  try doDrop(event, editor, dropCaret) { dataTransfer, selection, _ in
    try insertDataTransferForPlainText(dataTransfer, selection)
  }
}

/// `$insertGeneratedNodes`: inserts `nodes` at `selection`, unless a
/// `SELECTION_INSERT_CLIPBOARD_NODES_COMMAND` handler takes them.
public func insertGeneratedNodes(
  _ editor: Editor, _ nodes: [LexicalNode], _ selection: any BaseSelection
) throws {
  if try !editor.dispatchCommand(
    SELECTION_INSERT_CLIPBOARD_NODES_COMMAND,
    InsertClipboardNodesPayload(nodes: nodes, selection: selection))
  {
    try selection.insertNodes(nodes)
    try updateSelectionOnInsert(selection)
  }
}

private func updateSelectionOnInsert(_ selection: any BaseSelection) throws {
  guard let selection = selection as? RangeSelection, selection.isCollapsed() else {
    return
  }
  let anchor = selection.anchor
  var nodeToInspect: LexicalNode? = nil

  let anchorCaret = try caretFromPoint(anchor, .previous)
  if anchorCaret.isText {
    nodeToInspect = anchorCaret.origin
  } else {
    let range = try getCaretRange(anchorCaret, getChildCaret(getRoot(), .next).getFlipped())
    let carets = try range.iterNodeCarets()
    while let caret = try carets.next() {
      if caret.origin is TextNode {
        nodeToInspect = caret.origin
        break
      } else if let element = caret.origin as? ElementNode, try !element.isInline() {
        break
      }
    }
  }

  if let text = nodeToInspect as? TextNode {
    let newFormat = try text.getFormat()
    let newStyle = try text.getStyle()
    if selection.format != newFormat || selection.style != newStyle {
      selection.format = newFormat
      selection.style = newStyle
      selection.dirty = true
    }
  }
}

@discardableResult
private func appendNodesToJSON(
  _ editor: Editor, _ selection: (any BaseSelection)?, _ currentNode: LexicalNode,
  _ targetArray: inout [JSONValue]
) throws -> Bool {
  var shouldInclude = try selection.map { try currentNode.isSelected($0) } ?? true
  // 'clone', not 'html': this builds the internal
  // `application/x-lexical-editor` payload.
  let shouldExclude = try (currentNode as? ElementNode)?.excludeFromCopy("clone") ?? false
  var target = currentNode

  if let selection, let text = target as? TextNode {
    target = try sliceSelectedTextNodeContent(selection, text, .clone)
  }
  // Route through the shared export so a selection honors the same form as
  // editorState.toJSON().
  var serializedNode = try exportNodeJSON(target)
  let children = try (target as? ElementNode)?.getChildren() ?? []
  // JS pushes into the serialized node's own children array.
  var childTarget = serializedNode["children"]?.arrayValue ?? []
  if let text = target as? TextNode, try text.getTextContentSize() == 0 {
    // If an uncollapsed selection ends or starts at the end of a line of
    // specialized TextNodes, such as code tokens, we get a 'blank' TextNode
    // here. We don't want it.
    shouldInclude = false
  }

  // An element host in a NodeSelection recurses into its children with a
  // nil selection, so the whole subtree serializes. Only a whole-host
  // NodeSelection promotes; a partial RangeSelection keeps slicing.
  let childSelection: (any BaseSelection)? =
    shouldInclude && selection is NodeSelection && currentNode is ElementNode ? nil : selection

  for childNode in children {
    let shouldIncludeChild = try appendNodesToJSON(
      editor, childSelection, childNode, &childTarget)
    if !shouldInclude, let element = currentNode as? ElementNode, shouldIncludeChild,
      try element.extractWithChild(childNode, selection, "clone")
    {
      shouldInclude = true
    }
  }
  if serializedNode.has("children") {
    serializedNode["children"] = .array(childTarget)
  }

  // Slots are shadow-root isolated, so each slot subtree of an included
  // host is copied whole.
  if shouldInclude && !shouldExclude {
    let slotNames = try getSlotNames(target)
    if !slotNames.isEmpty {
      var serializedSlots = JSONObject()
      for name in slotNames {
        guard let slotNode = try getSlot(target, name) else {
          throw LexicalError(
            "LexicalNode: Node \(type(of: target)) has slot \"\(name)\" but it resolved to no node during export."
          )
        }
        var slotArray: [JSONValue] = []
        try appendNodesToJSON(editor, nil, slotNode, &slotArray)
        // A whole-slot export must serialize to exactly the slot value node.
        let excluded = try (slotNode as? ElementNode)?.excludeFromCopy("clone") ?? false
        if slotArray.count != 1 || excluded {
          throw LexicalError(
            "LexicalNode: slot \"\(name)\" on \(type(of: target)) did not serialize to exactly the slot value node (got \(slotArray.count) nodes); a slot value must not be excluded from copy."
          )
        }
        serializedSlots[name] = slotArray[0]
      }
      serializedNode["$slots"] = .object(serializedSlots)
    }
  }

  if shouldInclude && !shouldExclude {
    targetArray.append(.object(serializedNode))
  } else {
    // Splice up whatever the recursion collected in this node's place.
    targetArray += childTarget
  }
  return shouldInclude
}

/// The result of `$generateJSONFromSelectedNodes`: the editor namespace
/// and the serialized nodes.
public struct SerializedSelectedNodes {
  public var namespace: String
  public var nodes: [BaseSerializedNode]

  public init(namespace: String, nodes: [BaseSerializedNode]) {
    self.namespace = namespace
    self.nodes = nodes
  }

  /// `{namespace, nodes}`, in that key order.
  public func toJSON() -> JSONObject {
    JSONObject([("namespace", .string(namespace)), ("nodes", .array(nodes.map { .object($0) }))])
  }
}

/// `$generateJSONFromSelectedNodes`: the Lexical JSON of the nodes inside
/// `selection` (all nodes for `nil`), with the editor's namespace.
public func generateJSONFromSelectedNodes(
  _ editor: Editor, _ selection: (any BaseSelection)?
) throws -> SerializedSelectedNodes {
  var nodes: [JSONValue] = []
  let root = try getRoot()
  // A selection wholly inside a slot subtree never includes its host, so
  // walk the selection's slot frame; outside slots this is the root.
  let slotFrame = try getSelectionSlotFrame(selection)
  let topLevelChildren = try ((slotFrame as? ElementNode) ?? root).getChildren()
  try INTERNAL_withExportMemo {
    for topLevelNode in topLevelChildren {
      try appendNodesToJSON(editor, selection, topLevelNode, &nodes)
    }
  }
  return SerializedSelectedNodes(
    namespace: editor._config.namespace, nodes: nodes.compactMap(\.objectValue))
}

/// `$generateNodesFromSerializedNodes`: the nodes of a clipboard payload,
/// of the classes registered on the active editor.
public func generateNodesFromSerializedNodes(
  _ serializedNodes: [BaseSerializedNode]
) throws -> [LexicalNode] {
  try serializedNodes.map(parseSerializedNode)
}

/// `copyToClipboard(editor, event, data)`: writes the selection (or
/// `data`) to `event`'s clipboard data in the `text/plain`, `text/html` and
/// `application/x-lexical-editor` formats, in an update. JS returns a
/// promise; this returns the result directly.
///
/// Without an event JS fires a `copy` event at the document to reach the
/// system clipboard. Native views have no such event, so Swift fills
/// `pasteboard`, which stands for the platform pasteboard the view then
/// writes, and returns `false` without one.
@discardableResult
public func copyToClipboard(
  _ editor: Editor, _ event: ClipboardEvent?, _ data: LexicalClipboardData? = nil,
  pasteboard: DataTransfer? = nil
) throws -> Bool {
  guard
    let event = event ?? pasteboard.map({ ClipboardEvent("copy", clipboardData: $0) })
  else {
    return false
  }
  var result = false
  try editor.update {
    result = try copyToClipboardEvent(editor, event, data)
  }
  return result
}

private func copyToClipboardEvent(
  _ editor: Editor, _ event: ClipboardEvent, _ data: LexicalClipboardData?
) throws -> Bool {
  var data = data
  if data == nil {
    // JS also declines when the DOM selection is outside the editor;
    // native views only send the editor's own copy.
    guard let selection = try getSelection(), !selection.isCollapsed() else {
      return false
    }
    data = try getClipboardDataFromSelection(selection)
  }
  event.preventDefault()
  guard let clipboardData = event.clipboardData, let data else {
    return false
  }
  setLexicalClipboardDataTransfer(clipboardData, data)
  return true
}

private let clipboardDataFunctions = ["text/html", "application/x-lexical-editor"]

/// `$getClipboardDataFromSelection`: `selection` serialized for each
/// MIME type of the editor's `GetClipboardDataExtension` (or the defaults).
public func getClipboardDataFromSelection(
  _ selection: (any BaseSelection)?
) throws -> LexicalClipboardData {
  try getClipboardDataWithConfigFromSelection(getExportConfig(), selection)
}

/// `$getClipboardDataFromSelection()`: the current selection serialized.
public func getClipboardDataFromSelection() throws -> LexicalClipboardData {
  try getClipboardDataFromSelection(getSelection())
}

/// `setLexicalClipboardDataTransfer`: `setData` on `clipboardData` for each
/// MIME type in `data`, clearing `text/html` and
/// `application/x-lexical-editor` when `data` has none.
public func setLexicalClipboardDataTransfer(
  _ clipboardData: DataTransfer, _ data: LexicalClipboardData
) {
  for k in clipboardDataFunctions where data[k] == nil {
    clipboardData.setData(k, "")
  }
  for (k, v) in data {
    clipboardData.setData(k, v)
  }
}

/// `ExportMimeTypeFunction`: serializes a selection (`nil` when there is
/// none) for one MIME type, or returns `nil` to omit it. `next` calls the
/// handler below in the stack.
public typealias ExportMimeTypeFunction =
  (_ selection: (any BaseSelection)?, _ next: () throws -> String?) throws -> String?

/// `ExportMimeTypeConfig`: a stack of `ExportMimeTypeFunction` per MIME
/// type; the last function runs first.
public typealias ExportMimeTypeConfig = MimeTypeMap<[ExportMimeTypeFunction]>

/// `GetClipboardDataConfig`.
public struct GetClipboardDataConfig {
  /// `$exportMimeType`: merged with `[...prev, ...override]` per MIME type.
  public var exportMimeType: ExportMimeTypeConfig

  public init(exportMimeType: ExportMimeTypeConfig = DEFAULT_EXPORT_MIME_TYPE) {
    self.exportMimeType = exportMimeType
  }
}

private func getExportConfig() throws -> ExportMimeTypeConfig {
  let dep = try getPeerDependencyFromEditor(
    getEditor(), GetClipboardDataExtension.name, as: type(of: GetClipboardDataExtension))
  return dep?.output ?? DEFAULT_EXPORT_MIME_TYPE
}

/// The default `ExportMimeTypeConfig`: `getLexicalContent`,
/// `getHtmlContent` and `selection.getTextContent()`.
nonisolated(unsafe) public let DEFAULT_EXPORT_MIME_TYPE: ExportMimeTypeConfig = [
  "application/x-lexical-editor": [
    { sel, next in
      if let sel { return try getLexicalContent(getEditor(), sel) }
      return try next()
    }
  ],
  "text/html": [
    { sel, next in
      if let sel { return try getHtmlContent(getEditor(), sel) }
      return try next()
    }
  ],
  "text/plain": [
    { sel, next in
      if let sel { return try sel.getTextContent() }
      return try next()
    }
  ],
]

private func getClipboardDataWithConfigFromSelection(
  _ exportMimeType: ExportMimeTypeConfig, _ selection: (any BaseSelection)?
) throws -> LexicalClipboardData {
  var clipboardData: LexicalClipboardData = ["text/plain": ""]
  for (k, fns) in exportMimeType {
    if let v = try callExportMimeTypeFunctionStack(fns, selection) {
      clipboardData[k] = v
    }
  }
  return clipboardData
}

private func callExportMimeTypeFunctionStack(
  _ fns: [ExportMimeTypeFunction], _ selection: (any BaseSelection)?
) throws -> String? {
  func callAt(_ i: Int) throws -> String? {
    guard fns.indices.contains(i) else { return nil }
    return try fns[i](selection) { try callAt(i - 1) }
  }
  return try callAt(fns.count - 1)
}

/// `$exportMimeTypeFromSelection(mimeType, selection)`: `selection`
/// serialized for one MIME type with the editor's configured stack, or
/// `nil` when no handler produced it.
public func exportMimeTypeFromSelection(
  _ mimeType: String, _ selection: (any BaseSelection)?
) throws -> String? {
  try callExportMimeTypeFunctionStack(getExportConfig()[mimeType] ?? [], selection)
}

/// `$exportMimeTypeFromSelection(mimeType)`: the current selection.
public func exportMimeTypeFromSelection(_ mimeType: String) throws -> String? {
  try exportMimeTypeFromSelection(mimeType, getSelection())
}

/// `GetClipboardDataExtension`: how the selection is serialized into
/// clipboard MIME types when copying or dragging out of the editor. Its
/// output is the merged `ExportMimeTypeConfig`.
public let GetClipboardDataExtension = defineExtension(
  name: "@lexical/clipboard/GetClipboardData",
  config: safeCast(GetClipboardDataConfig()),
  mergeConfig: { config, partial in
    var merged = shallowMergeConfig(config, partial)
    if let partialMimeType = partial.exportMimeType {
      var exportMimeType = config.exportMimeType
      for (k, v) in partialMimeType {
        exportMimeType[k] = (exportMimeType[k] ?? []) + v
      }
      merged.exportMimeType = exportMimeType
    }
    return merged
  },
  build: { _, config, _ in config.exportMimeType })
