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
