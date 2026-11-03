import Lexical

// The platform-neutral half of the text views (Q28, Q40): native input
// becomes the command JS would dispatch, and the view only ever shows what
// the model renders. UITextInput / NSTextInputClient call these methods.

/// The editing commands of the M1 slice, handled the way `registerRichText`
/// handles them, until LexicalRichText is ported and replaces this.
@discardableResult
public func registerTextKitSliceEditing(_ editor: Editor) -> () -> Void {
  let removers = [
    editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND,
      { text, _ in
        guard let selection = try getSelection() else { return false }
        try selection.insertText(text)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      DELETE_CHARACTER_COMMAND,
      { isBackward, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.deleteCharacter(isBackward)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      DELETE_WORD_COMMAND,
      { isBackward, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.deleteWord(isBackward)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      DELETE_LINE_COMMAND,
      { isBackward, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.deleteLine(isBackward)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      INSERT_PARAGRAPH_COMMAND,
      { _, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        _ = try selection.insertParagraph()
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      INSERT_LINE_BREAK_COMMAND,
      { selectStart, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.insertLineBreak(selectStart)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      FORMAT_TEXT_COMMAND,
      { format, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.formatText(format)
        return true
      }, COMMAND_PRIORITY_EDITOR),
  ]
  return { removers.forEach { $0() } }
}

/// Drives an editor from a text view: input in, edits to the rendered text
/// and selection out.
public final class TextKitInputController {
  public let editor: Editor
  public let movement = TextKitMovement()
  /// Keeps `document` in step with the editor's commits.
  public let reconciler = TextKitReconciler()
  /// The document last rendered, which the view shows.
  public var document: TextKitDocument { reconciler.document }
  /// Called after every commit with the edits that bring the view's text
  /// from the previous rendering to `document`, and the selected range. A
  /// full render (`setEditorState`) is one edit replacing everything.
  public var onRender:
    ((_ edits: [TextKitEdit], _ selectedRange: (location: Int, length: Int)?) -> Void)?
  private var removeListener: (() -> Void)?

  public init(editor: Editor) {
    self.editor = editor
    editor.selectionMovement = movement
    removeListener = editor.registerUpdateListener { [weak self] payload in
      try self?.render(payload)
    }
    do {
      try editor.update(discrete: true) {
        let root = try getRoot()
        if try root.getChildrenSize() == 0 {
          try root.append(createParagraphNode())
        }
        if try getSelection() == nil {
          _ = try root.selectEnd()
        }
      }
      if reconciler.editorState !== editor.getEditorState() {
        try reconciler.render(editor.getEditorState())
      }
    } catch {
      try? editor._onError(error)
    }
  }

  deinit { removeListener?() }

  private func render(_ payload: UpdateListenerPayload) throws {
    let edits = try reconciler.reconcile(payload)
    let range = try payload.editorState.read { try selectedRange(getSelection()) }
    onRender?(edits, range)
  }

  private func selectedRange(_ selection: (any BaseSelection)?) -> (location: Int, length: Int)? {
    guard let selection = selection as? RangeSelection,
      let a = reconciler.document.location(for: selection.anchor),
      let f = reconciler.document.location(for: selection.focus)
    else { return nil }
    return (min(a, f), abs(a - f))
  }

  /// The selected range in the rendered text.
  public func selectedRange() throws -> (location: Int, length: Int)? {
    try editor.read(.latest) { try selectedRange(getSelection()) }
  }

  /// The view's selection changed (a tap, a click, arrow keys): select the
  /// model points at those locations.
  public func setSelectedRange(location: Int, length: Int) throws {
    // Mapped before the update, so the reconciler's document is not shared
    // (and copied) while the commit edits it.
    let anchor = document.point(at: location)
    let focus = document.point(at: location + length)
    try editor.update(discrete: true) {
      try selectPoints(anchor, focus)
    }
  }

  private func dispatch<P>(_ command: LexicalCommand<P>, _ payload: P) throws {
    try editor.update(discrete: true) {
      _ = try self.editor.dispatchCommand(command, payload)
    }
  }

  /// `insertText(_:)`: typed text, which replaces the selection.
  public func insertText(_ text: String) throws {
    // A newline from the keyboard is Enter, as `beforeinput` reports it.
    let lines = text.split(separator: "\n", omittingEmptySubsequences: false)
    for (i, line) in lines.enumerated() {
      if i > 0 { try dispatch(INSERT_PARAGRAPH_COMMAND, ()) }
      if !line.isEmpty { try dispatch(CONTROLLED_TEXT_INSERTION_COMMAND, String(line)) }
    }
  }

  public func deleteBackward() throws { try dispatch(DELETE_CHARACTER_COMMAND, true) }
  public func deleteForward() throws { try dispatch(DELETE_CHARACTER_COMMAND, false) }
  public func deleteWordBackward() throws { try dispatch(DELETE_WORD_COMMAND, true) }
  public func deleteToBeginningOfLine() throws { try dispatch(DELETE_LINE_COMMAND, true) }
  public func insertNewline() throws { try dispatch(INSERT_PARAGRAPH_COMMAND, ()) }
  public func insertLineBreak() throws { try dispatch(INSERT_LINE_BREAK_COMMAND, false) }
  public func toggleFormat(_ format: TextFormatType) throws {
    try dispatch(FORMAT_TEXT_COMMAND, format)
  }

  /// Moves or extends the selection by `granularity` (arrow keys).
  public func moveSelection(
    _ alter: SelectionAlter, isBackward: Bool, granularity: SelectionGranularity
  ) throws {
    try editor.update(discrete: true) {
      guard let selection = try getSelection() as? RangeSelection else { return }
      if alter == .move, !selection.isCollapsed() {
        let edge = try selection.isBackward() == isBackward ? selection.focus : selection.anchor
        let point = (edge.key, edge.offset, edge.type)
        try selectPoints(point, point)
        return
      }
      try selection.modify(alter, isBackward, granularity)
    }
  }
}
