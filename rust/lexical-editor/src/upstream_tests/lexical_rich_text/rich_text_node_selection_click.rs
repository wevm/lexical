//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionClick.test.ts`.
//! JS's click targets a DOM element; a Rust `MouseEvent` carries the key of
//! the node under the pointer as the view resolves it (`target_key`): the
//! decorator for a span inside its DOM, nothing for a detached `div` or a
//! `null` target.

#![cfg(feature = "extension")]

use super::support::{build_with_decorators, select_nodes};
use crate::extension::LexicalEditorWithDispose;
use crate::upstream_tests::support::{
    create_test_decorator_node_inline, set_test_root_element, update,
};
use crate::{
    CLICK_COMMAND, MouseEvent, NodeKey, NodeRef, create_paragraph_node, create_text_node, get_root,
    get_selection,
};
use std::rc::Rc;

fn create_editor(inline: bool) -> LexicalEditorWithDispose {
    let editor = build_with_decorators(move || {
        let decorator = create_test_decorator_node_inline(inline);
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("before")])?;
        if inline {
            // Inline decorator flows inside the paragraph, as in #8907.
            paragraph.append(&[decorator])?;
            get_root().append(&[paragraph])
        } else {
            // Block decorator sits at root level, next to the paragraph.
            get_root().append(&[paragraph, decorator])
        }
    });
    set_test_root_element(&editor);
    editor
}

/// The decorator is the last root child (block) or its last child (inline).
fn decorator() -> NodeRef {
    let last = get_root().get_last_child().unwrap();
    if last.is_element() {
        last.get_last_child().unwrap()
    } else {
        last
    }
}

fn select_decorator(editor: &LexicalEditorWithDispose) -> NodeKey {
    update(editor, || {
        select_nodes(&[decorator()]);
        Ok(())
    });
    editor.read(|| decorator().get_key().to_owned()).unwrap()
}

fn click(editor: &LexicalEditorWithDispose, target_key: Option<NodeKey>) -> bool {
    editor
        .dispatch_command(
            &CLICK_COMMAND,
            Rc::new(MouseEvent {
                target_key,
                ..MouseEvent::new("click")
            }),
        )
        .unwrap()
}

fn expect_node_selection_of(editor: &LexicalEditorWithDispose, key: &str) {
    editor
        .read(|| {
            let selection = get_selection()
                .filter(|s| s.is_node())
                .expect("expected a NodeSelection");
            assert!(selection.node_keys().iter().any(|k| k == key));
        })
        .unwrap();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionClick.test.ts::CLICK_COMMAND on a NodeSelection > inline decorator: click inside the node keeps the selection
#[test]
fn inline_decorator_click_inside_the_node_keeps_the_selection() {
    let editor = create_editor(true);
    let key = select_decorator(&editor);
    assert!(!click(&editor, Some(key.clone())));
    expect_node_selection_of(&editor, &key);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionClick.test.ts::CLICK_COMMAND on a NodeSelection > block decorator: click inside the node keeps the selection
#[test]
fn block_decorator_click_inside_the_node_keeps_the_selection() {
    let editor = create_editor(false);
    let key = select_decorator(&editor);
    assert!(!click(&editor, Some(key.clone())));
    expect_node_selection_of(&editor, &key);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionClick.test.ts::CLICK_COMMAND on a NodeSelection > click outside the selected node still deselects
#[test]
fn click_outside_the_selected_node_still_deselects() {
    let editor = create_editor(true);
    select_decorator(&editor);
    // A detached div is under no node.
    assert!(click(&editor, None));
    editor.read(|| assert!(get_selection().is_none())).unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionClick.test.ts::CLICK_COMMAND on a NodeSelection > click with a non-element target still deselects
#[test]
fn click_with_a_non_element_target_still_deselects() {
    let editor = create_editor(true);
    select_decorator(&editor);
    assert!(click(&editor, None));
    editor.read(|| assert!(get_selection().is_none())).unwrap();
    editor.dispose();
}
