//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionBackspace.test.ts`.
//! JS sets the event's `target` inside the decorator's DOM to exercise
//! the decorator pass-through; a Rust platform sends key events to the
//! editor itself (the pass-through has no counterpart), so the event has no
//! target.

#![cfg(feature = "extension")]

use super::support::{build_with_decorators, key_event, select_nodes};
use crate::extension::LexicalEditorWithDispose;
use crate::upstream_tests::support::{create_test_decorator_node_inline, update};
use crate::{
    KEY_BACKSPACE_COMMAND, KEY_DELETE_COMMAND, create_paragraph_node, create_text_node, get_root,
    get_selection,
};

fn create_editor(inline: bool) -> LexicalEditorWithDispose {
    build_with_decorators(move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("before")])?;
        get_root().append(&[paragraph, create_test_decorator_node_inline(inline)])
    })
}

fn select_decorator(editor: &LexicalEditorWithDispose) {
    update(editor, || {
        select_nodes(&[get_root().get_last_child().unwrap()]);
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionBackspace.test.ts::KEY_BACKSPACE_COMMAND on a single-decorator NodeSelection > block decorator: deletes the node
#[test]
fn backspace_block_decorator_deletes_the_node() {
    let editor = create_editor(false);
    select_decorator(&editor);
    let handled = editor
        .dispatch_command(&KEY_BACKSPACE_COMMAND, key_event("Backspace", false))
        .unwrap();
    assert!(handled);
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 1);
            assert!(!root.get_children()[0].is_decorator());
            assert!(get_selection().is_some_and(|s| s.is_range()));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionBackspace.test.ts::KEY_DELETE_COMMAND on a single-decorator NodeSelection > block decorator: deletes the node
#[test]
fn delete_block_decorator_deletes_the_node() {
    let editor = create_editor(false);
    select_decorator(&editor);
    let handled = editor
        .dispatch_command(&KEY_DELETE_COMMAND, key_event("Delete", false))
        .unwrap();
    assert!(handled);
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 1);
            assert!(!root.get_children()[0].is_decorator());
        })
        .unwrap();
    editor.dispose();
}
