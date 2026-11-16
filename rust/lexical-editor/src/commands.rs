//! `LexicalCommands.ts`: the core's own commands, which packages handle and
//! dispatch. Those whose payload is a DOM event carry the events of
//! `events` (`LexicalEvents.ts`), which an app makes from its own.
//!
//! They are constants, compared by id as every command is; their ids count
//! down from `u64::MAX`, where `create_command`'s count up from 1.

use crate::editor::LexicalCommand;
use crate::events::{
    ClipboardOrKeyboardEvent, CompositionEvent, DragEvent, FocusEvent, InputEvent, KeyboardEvent,
    MouseEvent, PasteCommandType,
};
use crate::lexical_selection::SelectionRef;
use crate::node::NodeRef;
use std::rc::Rc;

/// `UNDO_COMMAND`: dispatched on undo (⌘Z on macOS, Ctrl+Z elsewhere).
pub const UNDO_COMMAND: LexicalCommand<()> = LexicalCommand::constant(u64::MAX, "UNDO_COMMAND");

/// `REDO_COMMAND`: dispatched on redo (⇧⌘Z on macOS, Ctrl+Shift+Z or Ctrl+Y
/// elsewhere).
pub const REDO_COMMAND: LexicalCommand<()> = LexicalCommand::constant(u64::MAX - 1, "REDO_COMMAND");

/// `CLEAR_EDITOR_COMMAND`: dispatched to clear all editor content.
pub const CLEAR_EDITOR_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 2, "CLEAR_EDITOR_COMMAND");

/// `CLEAR_HISTORY_COMMAND`: dispatched to clear the undo and redo stacks.
pub const CLEAR_HISTORY_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 3, "CLEAR_HISTORY_COMMAND");

/// `CAN_REDO_COMMAND`: dispatched when whether redo is possible changes;
/// the payload is whether it is. JS deprecates it (v0.49.0) for
/// `HistoryExtension`'s `canRedo` signal (`HistoryExtensionOutput::can_redo`).
pub const CAN_REDO_COMMAND: LexicalCommand<bool> =
    LexicalCommand::constant(u64::MAX - 4, "CAN_REDO_COMMAND");

/// `CAN_UNDO_COMMAND`: dispatched when whether undo is possible changes;
/// the payload is whether it is. JS deprecates it (v0.49.0) for
/// `HistoryExtension`'s `canUndo` signal (`HistoryExtensionOutput::can_undo`).
pub const CAN_UNDO_COMMAND: LexicalCommand<bool> =
    LexicalCommand::constant(u64::MAX - 5, "CAN_UNDO_COMMAND");

/// `DELETE_CHARACTER_COMMAND`: dispatched to delete a character; the
/// payload is `true` for a backward deletion (Backspace) and `false` for a
/// forward one (Delete).
pub const DELETE_CHARACTER_COMMAND: LexicalCommand<bool> =
    LexicalCommand::constant(u64::MAX - 6, "DELETE_CHARACTER_COMMAND");

/// `INSERT_LINE_BREAK_COMMAND`: dispatched to insert a line break. With
/// `false` the caret moves to the new line (Shift+Enter); with `true` it
/// stays (Ctrl+O on macOS).
pub const INSERT_LINE_BREAK_COMMAND: LexicalCommand<bool> =
    LexicalCommand::constant(u64::MAX - 7, "INSERT_LINE_BREAK_COMMAND");

/// `INSERT_PARAGRAPH_COMMAND`: dispatched to insert a new paragraph
/// (Enter).
pub const INSERT_PARAGRAPH_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 8, "INSERT_PARAGRAPH_COMMAND");

/// `CONTROLLED_TEXT_INSERTION_COMMAND`: dispatched to insert text. JS also
/// takes the `beforeinput` event; without a DOM only the string form is
/// here.
pub const CONTROLLED_TEXT_INSERTION_COMMAND: LexicalCommand<String> =
    LexicalCommand::constant(u64::MAX - 9, "CONTROLLED_TEXT_INSERTION_COMMAND");

/// `REMOVE_TEXT_COMMAND`: dispatched to remove the selected text. JS's
/// payload is the input event or `null`, which no handler reads.
pub const REMOVE_TEXT_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 10, "REMOVE_TEXT_COMMAND");

/// `DELETE_WORD_COMMAND`: dispatched to delete a word; the payload is
/// `true` for a backward deletion (Opt+Delete on macOS).
pub const DELETE_WORD_COMMAND: LexicalCommand<bool> =
    LexicalCommand::constant(u64::MAX - 11, "DELETE_WORD_COMMAND");

/// `DELETE_LINE_COMMAND`: dispatched to delete a line; the payload is
/// `true` for a backward deletion (Cmd+Delete on macOS).
pub const DELETE_LINE_COMMAND: LexicalCommand<bool> =
    LexicalCommand::constant(u64::MAX - 12, "DELETE_LINE_COMMAND");

/// `FORMAT_TEXT_COMMAND`: dispatched to toggle a text format (`"bold"`,
/// `"italic"`, ...) on the selection.
pub const FORMAT_TEXT_COMMAND: LexicalCommand<String> =
    LexicalCommand::constant(u64::MAX - 13, "FORMAT_TEXT_COMMAND");

/// `SET_TEXT_FORMAT_COMMAND`: dispatched to set (`true`) or clear (`false`)
/// text formats on the selection. JS's payload is an object; its keys'
/// order is the order here.
pub const SET_TEXT_FORMAT_COMMAND: LexicalCommand<Vec<(String, bool)>> =
    LexicalCommand::constant(u64::MAX - 14, "SET_TEXT_FORMAT_COMMAND");

/// `INSERT_TAB_COMMAND`: dispatched to insert a tab.
pub const INSERT_TAB_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 15, "INSERT_TAB_COMMAND");

/// `INDENT_CONTENT_COMMAND`: dispatched to indent the selected content.
pub const INDENT_CONTENT_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 16, "INDENT_CONTENT_COMMAND");

/// `OUTDENT_CONTENT_COMMAND`: dispatched to outdent the selected content.
pub const OUTDENT_CONTENT_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 17, "OUTDENT_CONTENT_COMMAND");

/// `FORMAT_ELEMENT_COMMAND`: dispatched to set the alignment (`"left"`,
/// `"center"`, ...) of the selected blocks.
pub const FORMAT_ELEMENT_COMMAND: LexicalCommand<String> =
    LexicalCommand::constant(u64::MAX - 18, "FORMAT_ELEMENT_COMMAND");

/// `SELECT_ALL_COMMAND`: dispatched on the select all shortcut (⌘A on
/// macOS, Ctrl+A elsewhere), with the key event.
pub const SELECT_ALL_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 19, "SELECT_ALL_COMMAND");

/// `SELECTION_INSERT_CLIPBOARD_NODES_COMMAND`'s payload: the nodes a paste
/// made and the selection to insert them at.
#[derive(Clone, Debug)]
pub struct InsertClipboardNodesPayload {
    pub nodes: Vec<NodeRef>,
    pub selection: SelectionRef,
}

/// `SELECTION_INSERT_CLIPBOARD_NODES_COMMAND`: dispatched by
/// `$insertGeneratedNodes` (the `clipboard` feature) before it inserts the
/// nodes of a paste; a handler that returns `true` inserts them itself.
pub const SELECTION_INSERT_CLIPBOARD_NODES_COMMAND: LexicalCommand<InsertClipboardNodesPayload> =
    LexicalCommand::constant(u64::MAX - 26, "SELECTION_INSERT_CLIPBOARD_NODES_COMMAND");

/// `PASTE_COMMAND`: dispatched on a paste, with the `ClipboardEvent` (or
/// the `InputEvent` or `KeyboardEvent` that pasted).
pub const PASTE_COMMAND: LexicalCommand<PasteCommandType> =
    LexicalCommand::constant(u64::MAX - 27, "PASTE_COMMAND");

/// `COPY_COMMAND`: dispatched on a copy, with the `ClipboardEvent`, the
/// `KeyboardEvent` (⌘C on macOS, Ctrl+C elsewhere), or `None` (JS `null`).
pub const COPY_COMMAND: LexicalCommand<Option<ClipboardOrKeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 28, "COPY_COMMAND");

/// `CUT_COMMAND`: dispatched on a cut; the payload is as
/// [`COPY_COMMAND`]'s.
pub const CUT_COMMAND: LexicalCommand<Option<ClipboardOrKeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 29, "CUT_COMMAND");

/// `KEY_ENTER_COMMAND`: dispatched when Enter is pressed, with the key
/// event, or `None` (JS `null`) when the intent is to insert a newline
/// without one. Rich and plain text's handlers turn it into
/// `INSERT_PARAGRAPH_COMMAND` or `INSERT_LINE_BREAK_COMMAND`.
pub const KEY_ENTER_COMMAND: LexicalCommand<Option<Rc<KeyboardEvent>>> =
    LexicalCommand::constant(u64::MAX - 31, "KEY_ENTER_COMMAND");

/// `SELECTION_CHANGE_COMMAND`: dispatched in an update when the selection
/// changes. The core dispatches it from the DOM half of the update cycle,
/// which isn't ported (rust/PARITY.md), so only an app dispatches it.
pub const SELECTION_CHANGE_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 32, "SELECTION_CHANGE_COMMAND");

/// `CLICK_COMMAND`: dispatched on a click in the editor.
pub const CLICK_COMMAND: LexicalCommand<Rc<MouseEvent>> =
    LexicalCommand::constant(u64::MAX - 33, "CLICK_COMMAND");

/// `BEFORE_INPUT_COMMAND`: dispatched on a `beforeinput` event; the core's
/// handler (`$handleBeforeInput`) turns it into the command it stands for.
pub const BEFORE_INPUT_COMMAND: LexicalCommand<Rc<InputEvent>> =
    LexicalCommand::constant(u64::MAX - 34, "BEFORE_INPUT_COMMAND");

/// `INPUT_COMMAND`: dispatched on an `input` event.
pub const INPUT_COMMAND: LexicalCommand<Rc<InputEvent>> =
    LexicalCommand::constant(u64::MAX - 35, "INPUT_COMMAND");

/// `COMPOSITION_START_COMMAND`: dispatched when an input method
/// composition starts.
pub const COMPOSITION_START_COMMAND: LexicalCommand<Rc<CompositionEvent>> =
    LexicalCommand::constant(u64::MAX - 36, "COMPOSITION_START_COMMAND");

/// `COMPOSITION_END_COMMAND`: dispatched when an input method composition
/// ends.
pub const COMPOSITION_END_COMMAND: LexicalCommand<Rc<CompositionEvent>> =
    LexicalCommand::constant(u64::MAX - 37, "COMPOSITION_END_COMMAND");

/// `KEY_DOWN_COMMAND` (named `KEYDOWN_COMMAND`, as in JS): dispatched when
/// any key is pressed; the core's handler (`$handleKeyDown`) dispatches the
/// command the key stands for.
pub const KEY_DOWN_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 38, "KEYDOWN_COMMAND");

/// `KEY_ARROW_RIGHT_COMMAND`: dispatched when `ArrowRight` is pressed; Shift
/// may be down.
pub const KEY_ARROW_RIGHT_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 39, "KEY_ARROW_RIGHT_COMMAND");

/// `MOVE_TO_END`: dispatched on the move to end shortcut (⌘→ on macOS, End
/// elsewhere).
pub const MOVE_TO_END: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 40, "MOVE_TO_END");

/// `KEY_ARROW_LEFT_COMMAND`: dispatched when `ArrowLeft` is pressed; Shift
/// may be down.
pub const KEY_ARROW_LEFT_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 41, "KEY_ARROW_LEFT_COMMAND");

/// `MOVE_TO_START`: dispatched on the move to start shortcut (⌘← on macOS,
/// Home elsewhere).
pub const MOVE_TO_START: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 42, "MOVE_TO_START");

/// `KEY_ARROW_UP_COMMAND`: dispatched when `ArrowUp` is pressed; Shift and
/// Alt may be down.
pub const KEY_ARROW_UP_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 43, "KEY_ARROW_UP_COMMAND");

/// `KEY_ARROW_DOWN_COMMAND`: dispatched when `ArrowDown` is pressed; Shift
/// and Alt may be down.
pub const KEY_ARROW_DOWN_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 44, "KEY_ARROW_DOWN_COMMAND");

/// `KEY_SPACE_COMMAND`: dispatched when the space key is pressed, with any
/// modifiers.
pub const KEY_SPACE_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 45, "KEY_SPACE_COMMAND");

/// `KEY_BACKSPACE_COMMAND`: dispatched when `Backspace` is pressed; Shift
/// may be down.
pub const KEY_BACKSPACE_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 46, "KEY_BACKSPACE_COMMAND");

/// `KEY_ESCAPE_COMMAND`: dispatched when `Escape` is pressed, with any
/// modifiers.
pub const KEY_ESCAPE_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 47, "KEY_ESCAPE_COMMAND");

/// `KEY_DELETE_COMMAND`: dispatched when `Delete` is pressed (Fn+Delete on
/// macOS), with no modifiers.
pub const KEY_DELETE_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 48, "KEY_DELETE_COMMAND");

/// `KEY_TAB_COMMAND`: dispatched when `Tab` is pressed; Shift may be down.
pub const KEY_TAB_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 49, "KEY_TAB_COMMAND");

/// `KEY_MODIFIER_COMMAND`: dispatched after any key pressed with a
/// modifier. JS deprecates it (v0.31.0) for `KEY_DOWN_COMMAND`, checking
/// the modifiers.
pub const KEY_MODIFIER_COMMAND: LexicalCommand<Rc<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 50, "KEY_MODIFIER_COMMAND");

/// `DROP_COMMAND`: dispatched on a drop.
pub const DROP_COMMAND: LexicalCommand<Rc<DragEvent>> =
    LexicalCommand::constant(u64::MAX - 51, "DROP_COMMAND");

/// `DRAGSTART_COMMAND`: dispatched when a drag starts.
pub const DRAGSTART_COMMAND: LexicalCommand<Rc<DragEvent>> =
    LexicalCommand::constant(u64::MAX - 52, "DRAGSTART_COMMAND");

/// `DRAGOVER_COMMAND`: dispatched when a drag is over the editor.
pub const DRAGOVER_COMMAND: LexicalCommand<Rc<DragEvent>> =
    LexicalCommand::constant(u64::MAX - 53, "DRAGOVER_COMMAND");

/// `DRAGEND_COMMAND`: dispatched when a drag ends.
pub const DRAGEND_COMMAND: LexicalCommand<Rc<DragEvent>> =
    LexicalCommand::constant(u64::MAX - 54, "DRAGEND_COMMAND");

/// `FOCUS_COMMAND`: dispatched when the editor gets the focus.
pub const FOCUS_COMMAND: LexicalCommand<Rc<FocusEvent>> =
    LexicalCommand::constant(u64::MAX - 55, "FOCUS_COMMAND");

/// `BLUR_COMMAND`: dispatched when the editor loses the focus.
pub const BLUR_COMMAND: LexicalCommand<Rc<FocusEvent>> =
    LexicalCommand::constant(u64::MAX - 56, "BLUR_COMMAND");
