import Lexical
import LexicalText

// The platform-neutral half of the text views (Q28, Q40). The views turn
// each native input into the DOM-shaped event a browser would send and hand
// it here: key presses become `KEY_DOWN_COMMAND`, editing becomes
// `BEFORE_INPUT_COMMAND` with the `beforeinput` input type (which the
// editor's default handler turns into the commands `RichTextExtension`,
// `PlainTextExtension` and `HistoryExtension` handle), marked text goes
// through `COMPOSITION_START_COMMAND` and `COMPOSITION_END_COMMAND`, and the
// clipboard and drag and drop dispatch their commands with a
// `DataTransfer`. The view only ever shows what the model renders.

/// A range of the rendered string, in UTF-16 code units.
public struct TextKitRange: Equatable, Sendable {
  public var location: Int
  public var length: Int
  public var end: Int { location + length }

  public init(location: Int, length: Int) {
    self.location = location
    self.length = length
  }
}

/// Drives an editor from a text view: input in, edits to the rendered text
/// and selection out.
public final class TextKitInputController {
  public let editor: Editor
  public let movement = TextKitMovement()
  /// Keeps `document` in step with the editor's commits.
  public let reconciler = TextKitReconciler()
  /// The look of the rendered text; set it before the view first renders.
  public var theme: TextKitTheme = .default
  /// The document last rendered, which the view shows.
  public var document: TextKitDocument { reconciler.document }
  /// The quote bars of the blocks that have them, by block key.
  public private(set) var blockBars: [NodeKey: [TextKitBlockBar]] = [:]
  /// The list markers of the list items, by block key.
  public private(set) var listMarkers: [NodeKey: TextKitListMarkerLayout] = [:]
  /// The boxes of the blocks with a background, such as code blocks, by
  /// block key.
  public private(set) var blockBoxes: [NodeKey: TextKitBlockBoxLayout] = [:]
  /// The IME's marked text in the rendered string, while it composes.
  public private(set) var markedRange: TextKitRange?
  /// Called after every commit with what changed: the reconciler's edits,
  /// styled, the paragraphs and decorators to style again, the selection
  /// and the update's tags.
  public var onRender: ((TextKitUpdate) -> Void)?
  private var removeListener: (() -> Void)?

  public init(editor: Editor) {
    self.editor = editor
    editor.selectionMovement = movement
    movement.committedDocument = { [weak self, weak editor] in
      guard let self, let editor, reconciler.editorState === editor.getEditorState() else {
        return nil
      }
      return reconciler.document
    }
    removeListener = editor.registerUpdateListener { [weak self] payload in
      try self?.render(payload)
    }
    report {
      try editor.update(discrete: true) {
        let root = try getRoot()
        if try root.getChildrenSize() == 0 {
          try root.append(createParagraphNode())
        }
      }
      if reconciler.editorState !== editor.getEditorState() {
        try reconciler.render(editor.getEditorState())
      }
    }
  }

  deinit { removeListener?() }

  /// Runs `body`, sending what it throws to the editor's `onError`.
  @discardableResult
  public func report<T>(_ body: () throws -> T) -> T? {
    do {
      return try body()
    } catch {
      try? editor._onError(error)
      return nil
    }
  }

  private func render(_ payload: UpdateListenerPayload) throws {
    let edits = try reconciler.reconcile(payload)
    let isFull =
      edits.count == 1 && edits[0].location == 0
      && edits[0].string.utf16.count == reconciler.document.length
    try publish(
      edits, payload.editorState, tags: Set(payload.tags),
      dirtyElements: isFull ? nil : payload.dirtyElements, dirtyLeaves: payload.dirtyLeaves)
  }

  /// Renders the whole document again, as one edit replacing the view's
  /// text of `length` UTF-16 code units, and calls `onRender` with it. A
  /// view calls this when it first shows the editor, which may already
  /// have content.
  @discardableResult
  public func renderAll(replacing oldLength: Int) throws -> TextKitUpdate {
    let editorState = editor.getEditorState()
    var edits = try reconciler.render(editorState)
    edits[0] = TextKitEdit(
      location: 0, length: oldLength, string: edits[0].string, runs: edits[0].runs)
    return try publish(edits, editorState, tags: [], dirtyElements: nil)
  }

  @discardableResult
  private func publish(
    _ edits: [TextKitEdit], _ editorState: EditorState, tags: Set<String>,
    dirtyElements: JSMap<NodeKey, Bool>?, dirtyLeaves: JSSet<NodeKey>? = nil
  ) throws -> TextKitUpdate {
    let theme = theme
    let editor = editor
    let document = reconciler.document
    var update = try editorState.read {
      var update = try theme.update(
        edits, document, editor: editor, dirtyElements: dirtyElements, dirtyLeaves: dirtyLeaves)
      let selection = try getSelection()
      update.selectedRange = try Self.range(of: selection, in: document)
      if let selection = selection as? NodeSelection {
        update.selectedNodes = Set(try selection.getNodes().map { $0.getKey() })
      }
      // A document of more than one block never shows it, which settles
      // most commits without reading the whole text.
      update.canShowPlaceholder =
        try getRoot().getChildrenSize() <= 1 && canShowPlaceholder(editor.isComposing())
      return update
    }
    update.tags = tags
    if update.isFull {
      blockBars = [:]
      listMarkers = [:]
      blockBoxes = [:]
    }
    for paragraph in update.paragraphs {
      blockBars[paragraph.key] =
        paragraph.attributes.blockBars.isEmpty ? nil : paragraph.attributes.blockBars
      listMarkers[paragraph.key] = paragraph.attributes.listMarker.map {
        TextKitListMarkerLayout(
          marker: $0, headIndent: paragraph.attributes.headIndent, text: paragraph.text,
          isRightToLeft: paragraph.attributes.writingDirection == .rightToLeft)
      }
      blockBoxes[paragraph.key] = paragraph.attributes.blockBox.map {
        TextKitBlockBoxLayout(
          box: $0, text: paragraph.text,
          isRightToLeft: paragraph.attributes.writingDirection == .rightToLeft)
      }
    }
    // A block that left the document takes its marker, bars and box with
    // it.
    for key in listMarkers.keys where document.span(key) == nil { listMarkers[key] = nil }
    for key in blockBars.keys where document.span(key) == nil { blockBars[key] = nil }
    for key in blockBoxes.keys where document.span(key) == nil { blockBoxes[key] = nil }
    if let marked = markedRange, marked.end > document.length {
      markedRange = nil
    }
    onRender?(update)
    return update
  }

  private static func range(
    of selection: BaseSelection?, in document: TextKitDocument
  ) throws -> TextKitRange? {
    if let selection = selection as? RangeSelection {
      guard let a = document.location(for: selection.anchor),
        let f = document.location(for: selection.focus)
      else { return nil }
      return TextKitRange(location: min(a, f), length: abs(a - f))
    }
    // A selected decorator shows as its attachment character selected.
    if let selection = selection as? NodeSelection, let node = try selection.getNodes().first,
      let span = document.span(node.getKey())
    {
      return TextKitRange(location: span.location, length: span.length)
    }
    return nil
  }

  /// The selected range in the rendered text.
  public func selectedRange() throws -> TextKitRange? {
    try editor.read(.latest) { try Self.range(of: getSelection(), in: reconciler.document) }
  }

  /// Runs an update that dispatches native input, commits it at once and
  /// runs the microtasks it queued, as the end of a browser task does.
  private func run(_ body: @escaping () throws -> Void) throws {
    defer { drainMicrotasks() }
    try editor.update(discrete: true, body)
  }

  // MARK: Selection

  /// The view's selection changed (a click, a tap, an arrow key): the model
  /// half of JS `onSelectionChange`, which selects the points at those
  /// locations and gives a caret the format and style of where it is.
  /// `keepsAnchor` is whether the change extends the selection (a
  /// shift-arrow); a view passes `false` for a selection made another way,
  /// such as by a program or a click, which is forward.
  public func setSelectedRange(location: Int, length: Int, keepsAnchor: Bool = true) throws {
    // A view's range has no direction. A selection that still ends where
    // the model's anchor was is taken to extend from it, as a shift-arrow
    // does, so it is backward when that anchor is its end.
    let end = location + length
    let previousAnchor =
      keepsAnchor
      ? try editor.read(.latest) {
        (try getSelection() as? RangeSelection).flatMap { document.location(for: $0.anchor) }
      } : nil
    let isBackward = length > 0 && previousAnchor == end
    // Mapped before the update, so the reconciler's document is not shared
    // (and copied) while the commit edits it.
    let anchor = document.point(at: isBackward ? end : location)
    let focus = document.point(at: isBackward ? location : end)
    try run { [self] in
      try selectPoints(anchor, focus)
      guard let selection = try getSelection() as? RangeSelection else { return }
      if selection.isCollapsed() {
        try updateCollapsedSelectionFormatStyle(selection)
      } else {
        try updateRangeSelectionFormat(selection)
      }
      try dispatchSelectionChangeCommand(editor, selection, force: selection.dirty)
    }
  }

  /// `$updateCollapsedSelectionFormatStyle`: a caret takes the format and
  /// style of where it is.
  private func updateCollapsedSelectionFormatStyle(_ selection: RangeSelection) throws {
    let anchor = selection.anchor
    let anchorNode = try anchor.getNode()
    if let text = anchorNode as? TextNode {
      try updateSelectionFormatStyle(selection, text.getFormat(), text.getStyle())
    } else if let element = anchorNode as? ElementNode,
      try editor.isComposing() || getRoot().getTextContent() != ""
    {
      if try element.isEmpty() {
        try internalRefreshSelectionFormatAndStyle(selection, "")
      } else {
        updateSelectionFormatStyle(selection, selection.format, "")
      }
    }
  }

  private func updateSelectionFormatStyle(
    _ selection: RangeSelection, _ format: Int, _ style: String
  ) {
    if selection.format != format || selection.style != style {
      selection.format = format
      selection.style = style
      selection.dirty = true
    }
  }

  /// The formats every selected text node has, as JS `onSelectionChange`
  /// computes for a range.
  private func updateRangeSelectionFormat(_ selection: RangeSelection) throws {
    let nodes = try selection.getNodes()
    let isBackward = try selection.isBackward()
    let start = isBackward ? selection.focus : selection.anchor
    let end = isBackward ? selection.anchor : selection.focus
    var combinedFormat = IS_ALL_FORMATTING
    var hasTextNodes = false
    for (i, node) in nodes.enumerated() {
      let size = try node.getTextContentSize()
      guard let text = node as? TextNode, size != 0,
        !(i == 0 && text.getKey() == start.key && start.offset == size),
        !(i == nodes.count - 1 && text.getKey() == end.key && end.offset == 0)
      else { continue }
      hasTextNodes = true
      combinedFormat &= try text.getFormat()
      if combinedFormat == 0 { break }
    }
    selection.format = hasTextNodes ? combinedFormat : 0
  }

  // MARK: Keys and input

  /// `onKeyDown`: dispatches `KEY_DOWN_COMMAND`, unless an IME is
  /// composing. Returns whether a handler prevented the default, in which
  /// case the view must not act on the key itself.
  @discardableResult
  public func keyDown(_ event: KeyboardEvent) throws -> Bool {
    editor._inputState.lastKeyCode = event.key
    if editor.isComposing() || markedRange != nil { return false }
    try run { [self] in try dispatchCommand(editor, KEY_DOWN_COMMAND, event) }
    return event.defaultPrevented
  }

  /// A `beforeinput` of `inputType`, for `targetRange` of the rendered text
  /// (the current selection when `nil`). Returns whether it was handled.
  @discardableResult
  public func beforeInput(
    _ inputType: String, data: String? = nil, dataTransfer: DataTransfer? = nil,
    targetRange: TextKitRange? = nil
  ) throws -> Bool {
    let event = InputEvent(inputType: inputType, data: data, dataTransfer: dataTransfer)
    let target = targetRange.map { (document.point(at: $0.location), document.point(at: $0.end)) }
    try run { [self] in
      if let target {
        try selectPoints(target.0, target.1)
      }
      try dispatchCommand(editor, BEFORE_INPUT_COMMAND, event)
    }
    return event.defaultPrevented
  }

  /// Typed text, which replaces the selection (or `replacementRange`). A
  /// marked text in progress is committed with it.
  public func insertText(_ text: String, replacementRange: TextKitRange? = nil) throws {
    if markedRange != nil {
      try commitMarkedText(text)
      return
    }
    // A lone newline is Enter, however it arrives (the iOS keyboard's
    // Return replaces the selection with it), as Safari reports it.
    if text == "\n" {
      try beforeInput("insertParagraph", targetRange: replacementRange)
      return
    }
    let caret = try selectedRange()
    // A replacement of the selection is typing (the iOS keyboard sends each
    // key that way), as Safari reports it, so history merges it.
    if let replacementRange, replacementRange != caret {
      let transfer = DataTransfer()
      transfer.setData("text/plain", text)
      try beforeInput(
        "insertReplacementText", dataTransfer: transfer, targetRange: replacementRange)
      // A replacement before the caret (autocorrection, a spelling fix)
      // leaves the caret where it was, as the platform's text views do.
      if let caret, caret.length == 0, caret.location >= replacementRange.end {
        let delta = text.utf16.count - replacementRange.length
        try setSelectedRange(location: caret.location + delta, length: 0)
      }
    } else {
      try beforeInput("insertText", data: text)
    }
  }

  // MARK: Composition (IME marked text)

  /// The IME's marked text changed: `text` replaces the marked text (or
  /// `replacementRange`, or the selection, when composition starts), with
  /// the caret at `selectedRange` within it. The first call dispatches
  /// `COMPOSITION_START_COMMAND`; the text goes into the model, as a browser
  /// writes it into the DOM.
  public func setMarkedText(
    _ text: String, selectedRange: TextKitRange, replacementRange: TextKitRange? = nil
  ) throws {
    let currentSelection = try self.selectedRange()
    let target = markedRange ?? replacementRange ?? currentSelection
    let start = target?.location ?? 0
    let points = target.map { (document.point(at: $0.location), document.point(at: $0.end)) }
    try run { [self] in
      if let points {
        try selectPoints(points.0, points.1)
      }
      if !editor.isComposing() {
        try dispatchCommand(editor, COMPOSITION_START_COMMAND, CompositionEvent("compositionstart"))
      }
      guard let selection = try getSelection() as? RangeSelection else { return }
      if text.isEmpty {
        try selection.removeText()
      } else {
        try selection.insertText(text)
      }
      if let selection = try getSelection() as? RangeSelection {
        try setCompositionKey(selection.anchor.key)
      }
    }
    let length = text.utf16.count
    markedRange = length > 0 ? TextKitRange(location: start, length: length) : nil
    let caret = start + min(max(selectedRange.location, 0), length)
    let anchor = document.point(at: caret)
    let focus = document.point(at: caret + min(selectedRange.length, length - (caret - start)))
    try editor.update(discrete: true) {
      try selectPoints(anchor, focus)
    }
    if length == 0 { try endComposition("") }
  }

  /// `unmarkText()`: commits the marked text as it is.
  public func unmarkText() throws {
    guard let markedRange else {
      if editor.isComposing() { try endComposition("") }
      return
    }
    let text = String(
      decoding: document.string.utf16.dropFirst(markedRange.location).prefix(markedRange.length),
      as: UTF16.self)
    try commitMarkedText(text)
  }

  /// Ends the composition with `text` in place of the marked text, through
  /// `COMPOSITION_END_COMMAND`.
  private func commitMarkedText(_ text: String) throws {
    guard let marked = markedRange else { return }
    let current = String(
      decoding: document.string.utf16.dropFirst(marked.location).prefix(marked.length),
      as: UTF16.self)
    // A trailing newline is the Enter that ends the composition, which the
    // end handler dispatches.
    let composed = text.hasSuffix("\n") ? String(text.dropLast()) : text
    let start = document.point(at: marked.location)
    let end = document.point(at: marked.end)
    try run {
      try selectPoints(start, end)
      guard let selection = try getSelection() as? RangeSelection else { return }
      if composed != current {
        if composed.isEmpty {
          try selection.removeText()
        } else {
          try selection.insertText(composed)
        }
        // Text inserted while composing stays selected, as the browser's
        // composition range is; the commit leaves the caret after it.
        if let selection = try getSelection() as? RangeSelection {
          let focus = selection.focus
          try selection.anchor.set(focus.key, focus.offset, focus.type)
        }
      } else {
        try selectPoints(end, end)
      }
    }
    try endComposition(text)
  }

  private func endComposition(_ data: String) throws {
    markedRange = nil
    try run { [self] in
      try dispatchCommand(
        editor, COMPOSITION_END_COMMAND, CompositionEvent("compositionend", data: data))
    }
  }

  // MARK: Clipboard

  /// `copy`: what `COPY_COMMAND` put on the clipboard, or `nil` when no
  /// handler wrote it (then there is nothing to copy).
  public func copy() throws -> DataTransfer? {
    try clipboard("copy", COPY_COMMAND)
  }

  /// `cut`: as `copy`, and the handler removes the selection.
  public func cut() throws -> DataTransfer? {
    try clipboard("cut", CUT_COMMAND)
  }

  private func clipboard(
    _ type: String, _ command: LexicalCommand<ClipboardOrKeyboardEvent?>
  ) throws -> DataTransfer? {
    let event = ClipboardEvent(type)
    try run { [self] in try dispatchCommand(editor, command, .clipboard(event)) }
    return event.defaultPrevented ? event.clipboardData : nil
  }

  /// `paste`: dispatches `PASTE_COMMAND` with the pasteboard's contents.
  @discardableResult
  public func paste(_ dataTransfer: DataTransfer) throws -> Bool {
    let event = ClipboardEvent("paste", clipboardData: dataTransfer)
    var handled = false
    try run { [self] in handled = try dispatchCommand(editor, PASTE_COMMAND, .clipboard(event)) }
    return handled || event.defaultPrevented
  }

  // MARK: Drag and drop

  /// `dragstart`: the data to drag (the selection in every format), or
  /// `nil` when nothing is selected to drag.
  public func dragStart(x: Double, y: Double) throws -> DataTransfer? {
    let event = DragEvent("dragstart", clientX: x, clientY: y)
    try run { [self] in _ = try dispatchCommand(editor, DRAGSTART_COMMAND, event) }
    guard let dataTransfer = event.dataTransfer, !dataTransfer.types.isEmpty else { return nil }
    return dataTransfer
  }

  /// `dragover`: whether a handler accepted the drop here.
  @discardableResult
  public func dragOver(_ dataTransfer: DataTransfer, x: Double, y: Double) throws -> Bool {
    let event = DragEvent("dragover", dataTransfer: dataTransfer, clientX: x, clientY: y)
    try run { [self] in _ = try dispatchCommand(editor, DRAGOVER_COMMAND, event) }
    return event.defaultPrevented
  }

  /// `drop`: dispatches `DROP_COMMAND`. Returns whether it was handled.
  @discardableResult
  public func drop(_ dataTransfer: DataTransfer, x: Double, y: Double) throws -> Bool {
    let event = DragEvent("drop", dataTransfer: dataTransfer, clientX: x, clientY: y)
    var handled = false
    try run { [self] in handled = try dispatchCommand(editor, DROP_COMMAND, event) }
    return handled || event.defaultPrevented
  }

  /// `dragend`. A drag that another editor took as a move removes the
  /// dragged range here, where JS lets the drop side dispatch
  /// `deleteByDrag` at this editor's root.
  public func dragEnd(isMoveToOtherEditor: Bool) throws {
    try run { [self] in
      _ = try dispatchCommand(editor, DRAGEND_COMMAND, DragEvent("dragend"))
    }
    if isMoveToOtherEditor {
      try beforeInput("deleteByDrag")
    }
  }

  // MARK: Focus and clicks

  /// The view became (`true`) or stopped being the first responder.
  public func focusChanged(_ hasFocus: Bool) throws {
    try run { [self] in
      _ = try dispatchCommand(
        editor, hasFocus ? FOCUS_COMMAND : BLUR_COMMAND, FocusEvent(hasFocus ? "focus" : "blur"))
    }
  }

  /// `click`, after the view has moved the selection: `targetKey` is the
  /// node under the pointer (`"root"` past the last block).
  public func click(_ event: MouseEvent) throws {
    try run { [self] in _ = try dispatchCommand(editor, CLICK_COMMAND, event) }
  }

  /// Toggles check list item `key`, as `checkList.ts`'s click on its
  /// `::before` box does. TextKit reads list items through their JSON
  /// (it does not depend on `LexicalList`), so the toggle writes the
  /// flipped `checked` back the same way.
  public func toggleChecked(_ key: NodeKey) throws {
    try run {
      guard let node = try getNodeByKey(key) else { return }
      var json = try node.exportJSON()
      guard let checked = json["checked"]?.boolValue else { return }
      json["checked"] = .bool(!checked)
      _ = try node.getWritable().updateFromJSON(json)
    }
  }

  /// The check list item whose check box column contains `x`, for a click
  /// at UTF-16 `location` of the rendered text: the innermost item there,
  /// as `checkList.ts` takes the `<li>` under the pointer. `x` is in line
  /// coordinates, where a line starts at 0 and is `lineWidth` wide. (A
  /// touch view puts a control over each box instead.)
  public func checkboxItem(at x: Double, location: Int, lineWidth: Double) -> NodeKey? {
    var best: (key: NodeKey, length: Int, layout: TextKitListMarkerLayout)?
    for (key, layout) in listMarkers {
      guard let span = document.span(key), span.location <= location, location <= span.end,
        best.map({ span.length < $0.length }) ?? true
      else { continue }
      best = (key, span.length, layout)
    }
    guard let best, case .checkbox = best.layout.marker else { return nil }
    let (start, side) = best.layout.checkboxOffset(fontSize: best.layout.text.fontSize)
    let minX = best.layout.isRightToLeft ? lineWidth - start - side : start
    return x > minX && x < minX + side ? best.key : nil
  }

  /// The caret at `location` of the rendered text (call it in a read or
  /// update), for `LexicalRootElement.caretFromPoint`.
  public func caret(at location: Int) throws -> PointCaret? {
    let (key, offset, type) = document.point(at: location)
    guard let node = try getNodeByKey(key) else { return nil }
    if type == .text, let text = node as? TextNode {
      return try getTextPointCaret(text, .next, offset)
    }
    guard let element = node as? ElementNode else { return nil }
    let child: PointCaret = getChildCaret(element as LexicalNode?, .next)!
    guard offset > 0, let previous = try element.getChildAtIndex(offset - 1) else {
      return child
    }
    let sibling: PointCaret = getSiblingCaret(previous as LexicalNode, .next)
    return sibling
  }

  /// The key of the node at `location`: its text node or decorator, or the
  /// innermost element containing it.
  public func nodeKey(at location: Int) -> NodeKey {
    var best: (NodeKey, Int)?
    for (key, span) in document.spans
    where span.location <= location && location < span.end
      && best.map({ span.length < $0.1 }) ?? true
    {
      best = (key, span.length)
    }
    return best?.0 ?? "root"
  }
}
