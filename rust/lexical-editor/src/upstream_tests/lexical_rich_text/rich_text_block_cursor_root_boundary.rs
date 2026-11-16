//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts`.

#![cfg(feature = "extension")]

use super::support::{build_with_decorators, key_event, select_nodes};
use crate::extension::LexicalEditorWithDispose;
use crate::upstream_tests::support::{create_test_decorator_node_inline, update};
use crate::{
    KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND, KEY_ARROW_RIGHT_COMMAND, KEY_ARROW_UP_COMMAND,
    PointType, create_paragraph_node, create_text_node, get_root, get_selection,
};

// Structure used across tests:
//   root
//     decorator (block)   — index 0
//     paragraph > text    — index 1
//     decorator (block)   — index 2
fn create_boundary_editor() -> LexicalEditorWithDispose {
    build_with_decorators(|| {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("text")])?;
        let root = get_root();
        root.clear()?;
        root.append(&[
            create_test_decorator_node_inline(false),
            paragraph,
            create_test_decorator_node_inline(false),
        ])
    })
}

fn expect_block_cursor_at(editor: &LexicalEditorWithDispose, offset: usize) {
    editor
        .read(|| {
            let s = get_selection().and_then(|s| s.range()).expect("a range");
            assert!(s.is_collapsed());
            assert_eq!(s.anchor.point_type, PointType::Element);
            assert_eq!(s.anchor.key, "root");
            assert_eq!(s.anchor.offset, offset);
        })
        .unwrap();
}

fn select_root(editor: &LexicalEditorWithDispose, anchor: usize, focus: usize) {
    update(editor, move || {
        get_root().select(Some(anchor), Some(focus)).map(drop)
    });
}

fn select_child(editor: &LexicalEditorWithDispose, index: usize) {
    update(editor, move || {
        select_nodes(&[get_root().get_child_at_index(index).unwrap()]);
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#8886) > ArrowUp at block cursor before the top decorator does not leave the editor
#[test]
fn arrow_up_at_block_cursor_before_the_top_decorator_does_not_leave_the_editor() {
    let editor = create_boundary_editor();
    select_child(&editor, 0);
    // First ArrowUp collapses the NodeSelection to a block cursor at root:0
    editor
        .dispatch_command(&KEY_ARROW_UP_COMMAND, key_event("ArrowUp", false))
        .unwrap();
    expect_block_cursor_at(&editor, 0);
    // Second ArrowUp must be handled so the selection cannot escape.
    let event = key_event("ArrowUp", false);
    assert!(
        editor
            .dispatch_command(&KEY_ARROW_UP_COMMAND, event.clone())
            .unwrap()
    );
    assert!(event.default_prevented());
    expect_block_cursor_at(&editor, 0);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#8886) > ArrowDown at block cursor after the bottom decorator does not leave the editor
#[test]
fn arrow_down_at_block_cursor_after_the_bottom_decorator_does_not_leave_the_editor() {
    let editor = create_boundary_editor();
    select_child(&editor, 2);
    editor
        .dispatch_command(&KEY_ARROW_DOWN_COMMAND, key_event("ArrowDown", false))
        .unwrap();
    expect_block_cursor_at(&editor, 3);
    let event = key_event("ArrowDown", false);
    assert!(
        editor
            .dispatch_command(&KEY_ARROW_DOWN_COMMAND, event.clone())
            .unwrap()
    );
    assert!(event.default_prevented());
    expect_block_cursor_at(&editor, 3);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > ArrowRight at the block cursor after the last block stays put
#[test]
fn arrow_right_at_the_block_cursor_after_the_last_block_stays_put() {
    let editor = create_boundary_editor();
    select_root(&editor, 3, 3);
    expect_block_cursor_at(&editor, 3);
    let event = key_event("ArrowRight", false);
    assert!(
        editor
            .dispatch_command(&KEY_ARROW_RIGHT_COMMAND, event.clone())
            .unwrap()
    );
    assert!(event.default_prevented());
    expect_block_cursor_at(&editor, 3);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > ArrowLeft at the block cursor before the first block stays put
#[test]
fn arrow_left_at_the_block_cursor_before_the_first_block_stays_put() {
    let editor = create_boundary_editor();
    select_root(&editor, 0, 0);
    expect_block_cursor_at(&editor, 0);
    let event = key_event("ArrowLeft", false);
    assert!(
        editor
            .dispatch_command(&KEY_ARROW_LEFT_COMMAND, event.clone())
            .unwrap()
    );
    assert!(event.default_prevented());
    expect_block_cursor_at(&editor, 0);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > ArrowRight at the block cursor before the last block is not consumed
#[test]
fn arrow_right_at_the_block_cursor_before_the_last_block_is_not_consumed() {
    let editor = create_boundary_editor();
    // root:2 sits between the paragraph and the trailing decorator.
    select_root(&editor, 2, 2);
    editor
        .dispatch_command(&KEY_ARROW_RIGHT_COMMAND, key_event("ArrowRight", false))
        .unwrap();
    // The decorator navigation selects the trailing decorator.
    editor
        .read(|| {
            let s = get_selection()
                .filter(|s| s.is_node())
                .expect("a NodeSelection");
            assert!(s.get_nodes().unwrap() == vec![get_root().get_child_at_index(2).unwrap()]);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > Shift+ArrowRight at the block cursor after the last block stays put
#[test]
fn shift_arrow_right_at_the_block_cursor_after_the_last_block_stays_put() {
    let editor = create_boundary_editor();
    select_root(&editor, 3, 3);
    let event = key_event("ArrowRight", true);
    assert!(
        editor
            .dispatch_command(&KEY_ARROW_RIGHT_COMMAND, event.clone())
            .unwrap()
    );
    assert!(event.default_prevented());
    expect_block_cursor_at(&editor, 3);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > Shift+ArrowLeft at the block cursor before the first block stays put
#[test]
fn shift_arrow_left_at_the_block_cursor_before_the_first_block_stays_put() {
    let editor = create_boundary_editor();
    select_root(&editor, 0, 0);
    let event = key_event("ArrowLeft", true);
    assert!(
        editor
            .dispatch_command(&KEY_ARROW_LEFT_COMMAND, event.clone())
            .unwrap()
    );
    assert!(event.default_prevented());
    expect_block_cursor_at(&editor, 0);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > Shift+ArrowLeft at the block cursor after the last block is not consumed
#[test]
fn shift_arrow_left_at_the_block_cursor_after_the_last_block_is_not_consumed() {
    let editor = create_boundary_editor();
    select_root(&editor, 3, 3);
    editor
        .dispatch_command(&KEY_ARROW_LEFT_COMMAND, key_event("ArrowLeft", true))
        .unwrap();
    editor
        .read(|| {
            let s = get_selection().and_then(|s| s.range()).expect("a range");
            assert!(!s.is_collapsed());
            assert_eq!(s.focus.offset, 2);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > ArrowRight with a non-collapsed selection at the root edge is not consumed
#[test]
fn arrow_right_with_a_non_collapsed_selection_at_the_root_edge_is_not_consumed() {
    let editor = create_boundary_editor();
    select_root(&editor, 0, 3);
    let event = key_event("ArrowRight", false);
    assert!(
        !editor
            .dispatch_command(&KEY_ARROW_RIGHT_COMMAND, event.clone())
            .unwrap()
    );
    assert!(!event.default_prevented());
    editor.dispose();
}
