//! `registerList.ts` and the model part of `checkList.ts`: the list
//! commands, and what `registerList` and `registerCheckList` register for
//! them.

use super::format_list::{
    handle_list_insert_paragraph, insert_list, remove_list, update_children_list_item_value,
};
use super::list_item_node::is_list_item_node;
use super::list_node::{ListType, is_list_node};
use crate::commands::INSERT_PARAGRAPH_COMMAND;
use crate::editor::{COMMAND_PRIORITY_LOW, LexicalCommand};
use crate::lexical_selection::{SelectionRef, Which, get_selection};
use crate::lexical_utils::get_node_by_key;
use crate::node::NodeKey;
use crate::{Editor, Unregister};

// Like the core's commands (`commands.rs`), these are constants, with ids
// counting down from where the core's stop.

/// `UPDATE_LIST_START_COMMAND`'s payload.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateListStartPayload {
    pub list_node_key: NodeKey,
    pub new_start: f64,
}

/// `UPDATE_LIST_START_COMMAND`: dispatched to set where an ordered list
/// starts counting.
pub const UPDATE_LIST_START_COMMAND: LexicalCommand<UpdateListStartPayload> =
    LexicalCommand::constant(u64::MAX - 20, "UPDATE_LIST_START_COMMAND");

/// `INSERT_UNORDERED_LIST_COMMAND`: dispatched to turn the selection into a
/// bullet list.
pub const INSERT_UNORDERED_LIST_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 21, "INSERT_UNORDERED_LIST_COMMAND");

/// `INSERT_ORDERED_LIST_COMMAND`: dispatched to turn the selection into a
/// numbered list.
pub const INSERT_ORDERED_LIST_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 22, "INSERT_ORDERED_LIST_COMMAND");

/// `REMOVE_LIST_COMMAND`: dispatched to turn the lists holding the
/// selection into paragraphs.
pub const REMOVE_LIST_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 23, "REMOVE_LIST_COMMAND");

/// `INSERT_CHECK_LIST_COMMAND` (`checkList.ts`): dispatched to turn the
/// selection into a check list.
pub const INSERT_CHECK_LIST_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 24, "INSERT_CHECK_LIST_COMMAND");

/// `RegisterListOptions`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RegisterListOptions {
    /// Whether a list Enter splits continues the numbering
    /// (`$handleListInsertParagraph(restoreNumbering)`).
    pub restore_numbering: bool,
}

fn merge(unregisters: Vec<Unregister>) -> Unregister {
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}

/// `registerList(editor, options)`: the list commands, Enter in an empty
/// item, and the transforms that keep an item's text format and style in
/// step with its first text, and returns what unregisters them.
///
/// JS also handles `KEY_BACKSPACE_COMMAND`, a DOM key event, by calling
/// `collapseAtStart` on the item at the caret; without an input layer that
/// waits, and `deleteCharacter` reaches the same `collapseAtStart`.
pub fn register_list(editor: &Editor, options: RegisterListOptions) -> Unregister {
    merge(vec![
        editor.register_command(
            &INSERT_ORDERED_LIST_COMMAND,
            |_, _| {
                insert_list(ListType::Number)?;
                Ok(true)
            },
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &UPDATE_LIST_START_COMMAND,
            |payload, _| {
                let Some(list_node) = get_node_by_key(&payload.list_node_key).filter(is_list_node)
                else {
                    return Ok(false);
                };
                if list_node.get_list_type() == Some(ListType::Number) {
                    list_node.set_list_start(payload.new_start);
                    update_children_list_item_value(&list_node);
                }
                Ok(true)
            },
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &INSERT_UNORDERED_LIST_COMMAND,
            |_, _| {
                insert_list(ListType::Bullet)?;
                Ok(true)
            },
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &REMOVE_LIST_COMMAND,
            |_, _| {
                remove_list()?;
                Ok(true)
            },
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &INSERT_PARAGRAPH_COMMAND,
            move |_, _| handle_list_insert_paragraph(options.restore_numbering),
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_node_transform("listitem", |node| {
            match node.get_first_child() {
                Some(first_child) => {
                    if first_child.is_text() {
                        let text = first_child.text_data();
                        let element = node.element_data();
                        if element.get_text_style() != text.get_style() {
                            node.set_text_style(text.get_style());
                        }
                        if element.get_text_format() != text.get_format() {
                            node.set_text_format(text.get_format());
                        }
                    }
                }
                None => {
                    // If it's empty, check the selection
                    if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
                        let range = selection.range().expect("a range selection");
                        let element = node.element_data();
                        if (range.style != element.get_text_style()
                            || range.format != element.get_text_format())
                            && range.is_collapsed()
                            && selection
                                .point(Which::Anchor)
                                .expect("a range")
                                .get_node()?
                                == *node
                        {
                            node.set_text_style(&range.style);
                            node.set_text_format(range.format);
                        }
                    }
                }
            }
            Ok(())
        }),
        editor.register_node_transform("text", |node| {
            if let Some(list_item_parent_node) = node.get_parent().filter(is_list_item_node)
                && list_item_parent_node.get_first_child().as_ref() == Some(node)
            {
                let text = node.text_data();
                let element = list_item_parent_node.element_data();
                if text.get_style() != element.get_text_style()
                    || text.get_format() != element.get_text_format()
                {
                    list_item_parent_node.set_text_style(text.get_style());
                    list_item_parent_node.set_text_format(text.get_format());
                }
            }
            Ok(())
        }),
    ])
}

/// `registerCheckList(editor)`: `INSERT_CHECK_LIST_COMMAND`, and returns
/// what unregisters it.
///
/// JS also moves DOM focus onto a check list item so that Space, the arrow
/// keys and Escape operate its checkbox, and toggles it on a click or tap
/// on its marker (`toggleChecked`, which is
/// [`NodeRef::toggle_checked`](crate::NodeRef::toggle_checked)); those wait
/// for an input layer, as does `disableTakeFocusOnClick`.
pub fn register_check_list(editor: &Editor) -> Unregister {
    editor.register_command(
        &INSERT_CHECK_LIST_COMMAND,
        |_, _| {
            insert_list(ListType::Check)?;
            Ok(true)
        },
        COMMAND_PRIORITY_LOW,
    )
}
