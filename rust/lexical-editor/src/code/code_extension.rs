//! `CodeExtension.ts`: what `CodeExtension` registers.

use super::code_node::{exit_code_node_on_enter, is_code_node};
use crate::{COMMAND_PRIORITY_LOW, Editor, KEY_ENTER_COMMAND, Unregister, get_selection};

/// `CodeExtension`'s `register(editor)`, without the extension: a
/// `KEY_ENTER_COMMAND` handler that leaves a code block after two line
/// breaks at its end (`$exitCodeNodeOnEnter`), and a transform that clears
/// the format of a tab in a code block. Returns what unregisters them.
pub fn register_code(editor: &Editor) -> Unregister {
    let unregister_enter = editor.register_command(
        &KEY_ENTER_COMMAND,
        |event, _| match get_selection().filter(|s| s.is_range()) {
            Some(selection) if exit_code_node_on_enter(&selection)?.is_some() => {
                if let Some(event) = event {
                    event.prevent_default();
                }
                Ok(true)
            }
            _ => Ok(false),
        },
        COMMAND_PRIORITY_LOW,
    );
    let unregister_tab = editor.register_node_transform("tab", |node| {
        if node.text_data().get_format() != 0.0
            && node.get_parent().is_some_and(|p| is_code_node(&p))
        {
            node.set_format(0.0);
        }
        Ok(())
    });
    Box::new(move || {
        unregister_enter();
        unregister_tab();
    })
}
