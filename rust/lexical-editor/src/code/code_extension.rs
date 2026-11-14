//! `CodeExtension.ts`: what `CodeExtension` registers.

use super::code_node::is_code_node;
use crate::{Editor, Unregister};

/// `CodeExtension`'s `register(editor)`, without the extension: a transform
/// that clears the format of a tab in a code block. Its other handler, of
/// `KEY_ENTER_COMMAND` (`$exitCodeNodeOnEnter`), reads a `KeyboardEvent`
/// and waits for an input layer; without the extension `CodeNode`'s
/// `insertNewAfter` exits the block itself, as in JS. Returns what
/// unregisters it.
pub fn register_code(editor: &Editor) -> Unregister {
    editor.register_node_transform("tab", |node| {
        if node.text_data().get_format() != 0.0
            && node.get_parent().is_some_and(|p| is_code_node(&p))
        {
            node.set_format(0.0);
        }
        Ok(())
    })
}
