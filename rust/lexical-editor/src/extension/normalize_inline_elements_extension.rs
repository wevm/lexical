//! `NormalizeInlineElementsExtension.ts`: the transform that removes an
//! inline element once it is empty.

use crate::{Editor, LexicalError, NodeRef, Unregister};

/// `deleteEmptyInline(node)`: removes `node` when it is an inline element
/// with no children. JS's `__DEV__` warning for a node whose `canBeEmpty()`
/// is true has no console here.
fn delete_empty_inline(node: &NodeRef) -> Result<(), LexicalError> {
    if node.is_element() && node.is_inline() && node.is_empty() {
        node.remove(false)?;
    }
    Ok(())
}

/// `NormalizeInlineElementsExtension`'s `register(editor)`, without the
/// extension and with `disabled: false`: adds `deleteEmptyInline` to the
/// transforms of every registered `ElementNode` class with an `isInline` of
/// its own (`LinkNode` and `AutoLinkNode` among the ported ones). As in JS
/// it goes straight into each class's transforms, without marking that
/// class's nodes dirty as `registerNodeTransform` does. Returns what
/// removes it again.
pub fn register_normalize_inline_elements(editor: &Editor) -> Unregister {
    let unregisters: Vec<Unregister> = editor
        .registered_nodes()
        .values()
        .filter(|class| (class.is_inline_element)())
        .map(|class| editor.add_node_transform(class.node_type, delete_empty_inline))
        .collect();
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}
