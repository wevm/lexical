//! `@lexical/plain-text` (the `plain-text` feature): `registerPlainText`'s
//! command handlers that act on the model, from
//! `packages/lexical-plain-text/src/index.ts`.
//!
//! Its copy, cut and paste handlers take the `DataTransfer` of the
//! platform's clipboard event. The handlers of the other DOM events (the
//! arrow keys, Backspace, Delete, Enter, drop and drag) wait for an input
//! layer. With the `extension` feature, [`plain_text_extension`] is
//! `PlainTextExtension`.

use crate::commands::{
    CONTROLLED_TEXT_INSERTION_COMMAND, DELETE_CHARACTER_COMMAND, DELETE_LINE_COMMAND,
    DELETE_WORD_COMMAND, INSERT_LINE_BREAK_COMMAND, INSERT_PARAGRAPH_COMMAND, KEY_ENTER_COMMAND,
    REMOVE_TEXT_COMMAND, SELECT_ALL_COMMAND,
};
use crate::commands::{COPY_COMMAND, CUT_COMMAND, PASTE_COMMAND};
use crate::editor::COMMAND_PRIORITY_EDITOR;
use crate::events::DataTransfer;
use crate::lexical_selection::{SelectionRef, get_selection};
use crate::selection_insert_nodes::select_all;
use crate::{CUT_TAG, PASTE_TAG, UpdateOptions};
use crate::{Editor, LexicalError, Unregister};
use std::rc::Rc;

/// `onCopyForPlainText(event, editor)`: in an update, the selection's HTML
/// (`$getHtmlContent`, with the `html` feature) as the clipboard data's
/// `text/html` and its text as `text/plain`, unless nothing is selected.
fn on_copy_for_plain_text(clipboard_data: Option<Rc<DataTransfer>>) -> Result<(), LexicalError> {
    let Some(editor) = crate::updates::get_active_editor() else {
        return Ok(());
    };
    #[cfg(feature = "html")]
    let html_editor = editor.clone();
    editor.update(
        move || {
            let selection = get_selection();
            if let (Some(selection), Some(clipboard_data)) = (selection, clipboard_data)
                && !selection.range().is_some_and(|r| r.is_collapsed())
            {
                #[cfg(feature = "html")]
                clipboard_data.set_data(
                    "text/html",
                    &crate::clipboard::get_html_content(&html_editor, Some(&selection))?,
                );
                clipboard_data.set_data("text/plain", &selection.get_text_content()?);
            }
            Ok(())
        },
        UpdateOptions::default(),
    )
}

/// `onPasteForPlainText(event, editor)`: in an update tagged `paste`, the
/// clipboard data at a range selection, with
/// `$insertDataTransferForPlainText`.
fn on_paste_for_plain_text(clipboard_data: Option<Rc<DataTransfer>>) -> Result<(), LexicalError> {
    let Some(editor) = crate::updates::get_active_editor() else {
        return Ok(());
    };
    editor.update(
        move || {
            if let (Some(clipboard_data), Some(selection)) = (clipboard_data, range_selection()) {
                crate::clipboard::insert_data_transfer_for_plain_text(&clipboard_data, &selection)?;
            }
            Ok(())
        },
        UpdateOptions {
            // PASTE_TAG gives the paste its own undo entry (#8609).
            tags: vec![PASTE_TAG.to_owned()],
            ..UpdateOptions::default()
        },
    )
}

/// `onCutForPlainText(event, editor)`: copies, then removes the range
/// selection in an update tagged `cut`.
fn on_cut_for_plain_text(
    clipboard_data: Option<Rc<DataTransfer>>,
    editor: &Editor,
) -> Result<(), LexicalError> {
    on_copy_for_plain_text(clipboard_data)?;
    editor.update(
        || {
            if let Some(selection) = range_selection() {
                selection.remove_text()?;
            }
            Ok(())
        },
        UpdateOptions {
            // CUT_TAG gives the cut its own undo entry (#8609).
            tags: vec![CUT_TAG.to_owned()],
            ..UpdateOptions::default()
        },
    )
}

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
/// unregisters them. Enter (`KEY_ENTER_COMMAND` and
/// `INSERT_PARAGRAPH_COMMAND`) inserts a line
/// break. `CONTROLLED_TEXT_INSERTION_COMMAND` takes only the string form,
/// and `SELECT_ALL_COMMAND` selects the whole document, or the slot frame
/// the caret is in.
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
            &KEY_ENTER_COMMAND,
            |event, editor| {
                if !get_selection().is_some_and(|s| s.is_range()) {
                    return Ok(false);
                }
                if let Some(event) = event {
                    // No beforeinput to defer to (JS's iOS and Safari path).
                    event.prevent_default();
                }
                editor.dispatch_command(&INSERT_LINE_BREAK_COMMAND, false)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &SELECT_ALL_COMMAND,
            |_, _| {
                // Scoped to the slot frame the caret is in, if any: a
                // whole-document select-all would escape the slot.
                match get_selection().filter(SelectionRef::is_range) {
                    Some(selection)
                        if crate::slot::get_slot_frame(
                            &selection
                                .point(crate::Which::Anchor)
                                .expect("a range")
                                .get_node()?,
                        )
                        .is_some() =>
                    {
                        crate::selection_insert_nodes::select_all_in(&selection)?;
                    }
                    _ => {
                        select_all()?;
                    }
                }
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        on_range(editor, &COPY_COMMAND, |_, clipboard_data| {
            on_copy_for_plain_text(clipboard_data.clone())
        }),
        editor.register_command(
            &CUT_COMMAND,
            |clipboard_data, editor| {
                if range_selection().is_none() {
                    return Ok(false);
                }
                on_cut_for_plain_text(clipboard_data.clone(), editor)?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        on_range(editor, &PASTE_COMMAND, |_, clipboard_data| {
            on_paste_for_plain_text(clipboard_data.clone())
        }),
    ];
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}

/// `PlainTextExtension`.
#[cfg(feature = "extension")]
pub type PlainTextExtensionType = crate::LexicalExtension;

#[cfg(feature = "extension")]
fn make_plain_text_extension() -> Rc<PlainTextExtensionType> {
    use crate::extension_core::{ExtensionRegisterState, LexicalExtension, define_extension};
    define_extension(LexicalExtension {
        conflicts_with: vec!["@lexical/rich-text".to_owned()],
        // Of JS's dependencies, `NormalizeInlineElementsExtension`;
        // `DragonExtension` and `NormalizeTripleClickSelectionExtension`
        // listen to the root element.
        dependencies: vec![crate::extension::normalize_inline_elements_extension().into()],
        register: Some(Rc::new(
            |editor: &Editor, _: &(), _: &ExtensionRegisterState<(), ()>| {
                Ok(register_plain_text(editor))
            },
        )),
        ..LexicalExtension::new("@lexical/plain-text", ())
    })
}

#[cfg(feature = "extension")]
thread_local! {
    static PLAIN_TEXT_EXTENSION: Rc<PlainTextExtensionType> = make_plain_text_extension();
}

/// `PlainTextExtension`: the plain text commands ([`register_plain_text`]),
/// with `NormalizeInlineElementsExtension`. It can't be built with
/// `RichTextExtension`.
#[cfg(feature = "extension")]
pub fn plain_text_extension() -> Rc<PlainTextExtensionType> {
    PLAIN_TEXT_EXTENSION.with(Rc::clone)
}
