//! `@lexical/plain-text` (the `plain-text` feature): `registerPlainText`'s
//! command handlers that act on the model, from
//! `packages/lexical-plain-text/src/index.ts`.
//!
//! The handlers of DOM events (the arrow keys, Backspace, Delete, Enter,
//! copy, cut, paste, drop and drag) wait for an input layer, and
//! `PlainTextExtension`'s dependencies (`DragonExtension`,
//! `NormalizeInlineElementsExtension`,
//! `NormalizeTripleClickSelectionExtension`) for the extension API.

use crate::commands::{
    CONTROLLED_TEXT_INSERTION_COMMAND, DELETE_CHARACTER_COMMAND, DELETE_LINE_COMMAND,
    DELETE_WORD_COMMAND, INSERT_LINE_BREAK_COMMAND, INSERT_PARAGRAPH_COMMAND, REMOVE_TEXT_COMMAND,
    SELECT_ALL_COMMAND,
};
use crate::editor::COMMAND_PRIORITY_EDITOR;
use crate::lexical_selection::{SelectionRef, get_selection};
use crate::selection_insert_nodes::select_all;
use crate::{Editor, LexicalError, Unregister};

/// The current selection when it is a range selection.
fn range_selection() -> Option<SelectionRef> {
    get_selection().filter(SelectionRef::is_range)
}

/// Registers `listener` for `command`, run with the range selection, and
/// not handled (`false`) without one, as every `registerPlainText` handler
/// does.
fn on_range<P: 'static>(
    editor: &Editor,
    command: &crate::LexicalCommand<P>,
    listener: impl Fn(&SelectionRef, &P) -> Result<(), LexicalError> + 'static,
) -> Unregister {
    editor.register_command(
        command,
        move |payload, _| {
            let Some(selection) = range_selection() else {
                return Ok(false);
            };
            listener(&selection, payload)?;
            Ok(true)
        },
        COMMAND_PRIORITY_EDITOR,
    )
}

/// `registerPlainText(editor)`: registers the plain text command handlers
/// that act on the model, at `COMMAND_PRIORITY_EDITOR`, and returns what
/// unregisters them. Enter (`INSERT_PARAGRAPH_COMMAND`) inserts a line
/// break. `CONTROLLED_TEXT_INSERTION_COMMAND` takes only the string form,
/// and `SELECT_ALL_COMMAND` always selects the whole document, as JS does
/// outside a slot (slots aren't ported).
pub fn register_plain_text(editor: &Editor) -> Unregister {
    let unregisters: Vec<Unregister> = vec![
        on_range(
            editor,
            &DELETE_CHARACTER_COMMAND,
            |selection, &is_backward| selection.delete_character(is_backward),
        ),
        on_range(editor, &DELETE_WORD_COMMAND, |selection, &is_backward| {
            selection.delete_word(is_backward)
        }),
        on_range(editor, &DELETE_LINE_COMMAND, |selection, &is_backward| {
            selection.delete_line(is_backward)
        }),
        on_range(
            editor,
            &CONTROLLED_TEXT_INSERTION_COMMAND,
            |selection, text| selection.insert_text(text),
        ),
        on_range(editor, &REMOVE_TEXT_COMMAND, |selection, _| {
            selection.remove_text()
        }),
        on_range(
            editor,
            &INSERT_LINE_BREAK_COMMAND,
            |selection, &select_start| selection.insert_line_break(select_start),
        ),
        on_range(editor, &INSERT_PARAGRAPH_COMMAND, |selection, _| {
            selection.insert_line_break(false)
        }),
        editor.register_command(
            &SELECT_ALL_COMMAND,
            |_, _| {
                select_all()?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
    ];
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}
