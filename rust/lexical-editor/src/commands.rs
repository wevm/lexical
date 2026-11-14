//! `LexicalCommands.ts`: the core's own commands, which packages handle and
//! dispatch. Only those a ported package uses are here.
//!
//! They are constants, compared by id as every command is; their ids count
//! down from `u64::MAX`, where `create_command`'s count up from 1.

use crate::editor::LexicalCommand;

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
/// `HistoryExtension`'s `canRedo` signal, which isn't ported yet.
pub const CAN_REDO_COMMAND: LexicalCommand<bool> =
    LexicalCommand::constant(u64::MAX - 4, "CAN_REDO_COMMAND");

/// `CAN_UNDO_COMMAND`: dispatched when whether undo is possible changes;
/// the payload is whether it is. JS deprecates it (v0.49.0) for
/// `HistoryExtension`'s `canUndo` signal, which isn't ported yet.
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

/// `SELECT_ALL_COMMAND`: dispatched to select all content. JS's payload is
/// the keyboard event, which no handler reads.
pub const SELECT_ALL_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 19, "SELECT_ALL_COMMAND");

/// The parts of a DOM `KeyboardEvent` that key command handlers read: the
/// modifier keys, and `preventDefault()`, which a handler calls when it
/// takes over the key. An app without a DOM makes one from its own key
/// event.
#[derive(Debug, Default)]
pub struct KeyboardEvent {
    pub shift_key: bool,
    pub alt_key: bool,
    pub ctrl_key: bool,
    pub meta_key: bool,
    default_prevented: std::cell::Cell<bool>,
}

impl KeyboardEvent {
    /// `event.preventDefault()`.
    pub fn prevent_default(&self) {
        self.default_prevented.set(true);
    }

    /// `event.defaultPrevented`.
    pub fn default_prevented(&self) -> bool {
        self.default_prevented.get()
    }
}

/// `KEY_ENTER_COMMAND`: dispatched when Enter is pressed, with the key
/// event, or `None` (JS `null`) when there is none. Rich and plain text's
/// handlers turn it into `INSERT_PARAGRAPH_COMMAND` or
/// `INSERT_LINE_BREAK_COMMAND`.
pub const KEY_ENTER_COMMAND: LexicalCommand<Option<KeyboardEvent>> =
    LexicalCommand::constant(u64::MAX - 26, "KEY_ENTER_COMMAND");
