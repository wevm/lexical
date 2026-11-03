import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalSelection

// Port of packages/lexical-plain-text/src/index.ts. As in `registerRichText`
// (DESIGN.md, "Rich text"), the `IS_IOS`/`IS_SAFARI` early returns of
// Backspace and Enter are left out: native views have no `beforeinput` to
// fall through to. So are the embedded-input checks of copy, cut and
// paste: a native control inside a decorator handles its own clipboard.

/// `onCopyForPlainText`.
private func onCopyForPlainText(_ event: ClipboardOrKeyboardEvent?, _ editor: Editor) throws {
  try editor.update {
    guard case .clipboard(let clipboardEvent) = event,
      let clipboardData = clipboardEvent.clipboardData,
      let selection = try getSelection(), !selection.isCollapsed()
    else { return }
    clipboardEvent.preventDefault()
    clipboardData.setData("text/html", try getHtmlContent(editor))
    clipboardData.setData("text/plain", try selection.getTextContent())
  }
}

/// `onPasteForPlainText`.
private func onPasteForPlainText(_ event: PasteCommandType, _ editor: Editor) throws {
  event.event.preventDefault()
  // PASTE_TAG gives the paste its own undo entry (#8609).
  try editor.update(tag: PASTE_TAG) {
    if case .clipboard(let clipboardEvent) = event,
      let clipboardData = clipboardEvent.clipboardData,
      let selection = try getSelection() as? RangeSelection
    {
      try insertDataTransferForPlainText(clipboardData, selection)
    }
  }
}

/// `onCutForPlainText`.
private func onCutForPlainText(_ event: ClipboardOrKeyboardEvent?, _ editor: Editor) throws {
  try onCopyForPlainText(event, editor)
  try editor.update(tag: CUT_TAG) {
    if let selection = try getSelection() as? RangeSelection {
      try selection.removeText()
    }
  }
}

/// `registerPlainText(editor)`: the plain text commands. Enter and
/// INSERT_PARAGRAPH insert a line break.
public func registerPlainText(_ editor: Editor) -> () -> Void {
  mergeRegister([
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
      CONTROLLED_TEXT_INSERTION_COMMAND,
      { text, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.insertText(text)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      REMOVE_TEXT_COMMAND,
      { _, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.removeText()
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
      INSERT_PARAGRAPH_COMMAND,
      { _, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.insertLineBreak()
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_ARROW_LEFT_COMMAND,
      { event, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        if try shouldOverrideDefaultCharacterSelection(selection, true) {
          event.preventDefault()
          try moveCharacter(selection, event.shiftKey, true)
          return true
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_ARROW_RIGHT_COMMAND,
      { event, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        if try shouldOverrideDefaultCharacterSelection(selection, false) {
          event.preventDefault()
          try moveCharacter(selection, event.shiftKey, false)
          return true
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_BACKSPACE_COMMAND,
      { event, editor in
        guard try getSelection() is RangeSelection else { return false }
        event.preventDefault()
        return try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_DELETE_COMMAND,
      { event, editor in
        guard try getSelection() is RangeSelection else { return false }
        event.preventDefault()
        return try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, false)
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_ENTER_COMMAND,
      { event, editor in
        guard try getSelection() is RangeSelection else { return false }
        event?.preventDefault()
        return try editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, false)
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      SELECT_ALL_COMMAND,
      { _, _ in
        // Scope select-all to the slot frame the caret is in: slots are
        // isolated, so a whole-document select-all would escape the slot.
        let selection = try getSelection() as? RangeSelection
        if let selection, try getSlotFrame(selection.anchor.getNode()) != nil {
          try selectAll(selection)
        } else {
          try selectAll(nil)
        }
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      COPY_COMMAND,
      { event, editor in
        guard try getSelection() is RangeSelection else { return false }
        try onCopyForPlainText(event, editor)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      CUT_COMMAND,
      { event, editor in
        guard try getSelection() is RangeSelection else { return false }
        try onCutForPlainText(event, editor)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      PASTE_COMMAND,
      { event, editor in
        guard try getSelection() is RangeSelection else { return false }
        try onPasteForPlainText(event, editor)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      DROP_COMMAND,
      { event, editor in
        let caret = try (editor.getRootElement() as? LexicalRootElement)?.caretFromPoint(
          event.clientX, event.clientY)
        return try handlePlainTextDrop(event, editor, caret)
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      DRAGSTART_COMMAND,
      { event, editor in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        // Mark the drag source so a drop in another editor can remove the
        // source range.
        if !selection.isCollapsed(), let dataTransfer = event.dataTransfer {
          writeDragSourceToDataTransfer(dataTransfer, editor)
        }
        return true
      }, COMMAND_PRIORITY_EDITOR),
  ])
}

/// `PlainTextExtension`. Of upstream's dependencies, Dragon and
/// NormalizeTripleClickSelection (browser events) are not ported
/// (DESIGN.md).
public let PlainTextExtension = defineExtension(
  name: "@lexical/plain-text", config: (), conflictsWith: ["@lexical/rich-text"],
  dependencies: [NormalizeInlineElementsExtension],
  register: { editor, _, _ in registerPlainText(editor) })
