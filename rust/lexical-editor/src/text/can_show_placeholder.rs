//! `packages/lexical-text/src/canShowPlaceholder.ts`.

use super::is_root_text_content_empty;
use crate::get_root;
use crate::nodes::paragraph::ParagraphNode;

/// `$canShowPlaceholder(isComposing)`: whether the input should show its
/// placeholder: the root has no text and at most one child, which is not a
/// decorator and, if an element, an unindented paragraph of text nodes.
pub fn can_show_placeholder(is_composing: bool) -> bool {
    if !is_root_text_content_empty(is_composing, false) {
        return false;
    }

    let root = get_root();
    let children = root.get_children();
    let children_length = children.len();

    if children_length > 1 {
        return false;
    }

    for top_block in &children {
        if top_block.is_decorator() {
            return false;
        }

        let latest = top_block.get_latest();
        if let Some(element) = latest.element() {
            if !latest.is::<ParagraphNode>() {
                return false;
            }

            if element.get_indent() != 0.0 {
                return false;
            }

            if top_block
                .get_children()
                .iter()
                .any(|child| !child.is_text())
            {
                return false;
            }
        }
    }

    true
}

/// `$canShowPlaceholderCurry(isEditorComposing)`: a function that runs
/// [`can_show_placeholder`] with it.
pub fn can_show_placeholder_curry(is_editor_composing: bool) -> impl Fn() -> bool {
    move || can_show_placeholder(is_editor_composing)
}
