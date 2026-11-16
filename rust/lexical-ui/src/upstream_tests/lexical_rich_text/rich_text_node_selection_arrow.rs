//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts`.

#![cfg(feature = "extension")]

use super::support::{build_with_decorators, key_event, select_nodes};
use crate::extension::LexicalEditorWithDispose;
use crate::upstream_tests::support::{create_test_decorator_node_inline, update};
use crate::{
    KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND, KEY_ARROW_RIGHT_COMMAND, KEY_ARROW_UP_COMMAND,
    KeyboardEvent, LexicalCommand, PointType, create_paragraph_node, create_text_node, get_root,
    get_selection,
};
use std::rc::Rc;

fn create_editor() -> LexicalEditorWithDispose {
    build_with_decorators(|| {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("text")])?;
        let root = get_root();
        root.clear()?;
        root.append(&[
            create_test_decorator_node_inline(false),
            create_test_decorator_node_inline(false),
            create_test_decorator_node_inline(false),
            paragraph,
        ])
    })
}

fn select_indexes(editor: &LexicalEditorWithDispose, indexes: &'static [usize]) {
    update(editor, move || {
        let root = get_root();
        let nodes: Vec<_> = indexes
            .iter()
            .map(|&i| root.get_child_at_index(i).unwrap())
            .collect();
        select_nodes(&nodes);
        Ok(())
    });
}

fn select_root(editor: &LexicalEditorWithDispose, anchor_offset: usize, focus_offset: usize) {
    update(editor, move || {
        get_root().select(Some(anchor_offset), Some(focus_offset))?;
        Ok(())
    });
}

/// `{defaultPrevented, handled}`.
fn dispatch_shift_arrow(
    editor: &LexicalEditorWithDispose,
    command: &LexicalCommand<Rc<KeyboardEvent>>,
    key: &str,
) -> (bool, bool) {
    let event = key_event(key, true);
    let handled = editor.dispatch_command(command, event.clone()).unwrap();
    (event.default_prevented(), handled)
}

fn expect_root_range(editor: &LexicalEditorWithDispose, anchor_offset: usize, focus_offset: usize) {
    editor
        .read(|| {
            let selection = get_selection().and_then(|s| s.range()).expect("a range");
            for (point, offset) in [
                (&selection.anchor, anchor_offset),
                (&selection.focus, focus_offset),
            ] {
                assert_eq!(point.key, "root");
                assert_eq!(point.offset, offset);
                assert_eq!(point.point_type, PointType::Element);
            }
        })
        .unwrap();
}

/// The `test.for` case.
fn run_case(
    command: &LexicalCommand<Rc<KeyboardEvent>>,
    key: &str,
    selected_indexes: &'static [usize],
    converted: (usize, usize),
    expected: (usize, usize),
    handled: bool,
) {
    let node_selection_editor = create_editor();
    select_indexes(&node_selection_editor, selected_indexes);
    let node_selection_result = dispatch_shift_arrow(&node_selection_editor, command, key);
    assert_eq!(node_selection_result, (handled, handled));
    expect_root_range(&node_selection_editor, expected.0, expected.1);

    // Starting from the converted RangeSelection produces the same outcome.
    let range_selection_editor = create_editor();
    select_root(&range_selection_editor, converted.0, converted.1);
    let range_selection_result = dispatch_shift_arrow(&range_selection_editor, command, key);
    assert_eq!(range_selection_result, node_selection_result);
    expect_root_range(&range_selection_editor, expected.0, expected.1);
    node_selection_editor.dispose();
    range_selection_editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowRight with [ 0 ] selected converts to [ 0, 1 ] and matches the equivalent RangeSelection
#[test]
fn arrow_right_with_0_selected_converts_to_0_1() {
    run_case(
        &KEY_ARROW_RIGHT_COMMAND,
        "ArrowRight",
        &[0],
        (0, 1),
        (0, 2),
        true,
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowDown with [ 0 ] selected converts to [ 0, 1 ] and matches the equivalent RangeSelection
#[test]
fn arrow_down_with_0_selected_converts_to_0_1() {
    run_case(
        &KEY_ARROW_DOWN_COMMAND,
        "ArrowDown",
        &[0],
        (0, 1),
        (0, 1),
        false,
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowLeft with [ 1 ] selected converts to [ 2, 1 ] and matches the equivalent RangeSelection
#[test]
fn arrow_left_with_1_selected_converts_to_2_1() {
    run_case(
        &KEY_ARROW_LEFT_COMMAND,
        "ArrowLeft",
        &[1],
        (2, 1),
        (2, 0),
        true,
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowUp with [ 1 ] selected converts to [ 2, 1 ] and matches the equivalent RangeSelection
#[test]
fn arrow_up_with_1_selected_converts_to_2_1() {
    run_case(
        &KEY_ARROW_UP_COMMAND,
        "ArrowUp",
        &[1],
        (2, 1),
        (2, 1),
        false,
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowUp with [ 0 ] selected converts to [ 1, 0 ] and matches the equivalent RangeSelection
#[test]
fn arrow_up_with_0_selected_converts_to_1_0() {
    run_case(&KEY_ARROW_UP_COMMAND, "ArrowUp", &[0], (1, 0), (1, 0), true);
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowRight with [ 1, 0 ] selected converts to [ 0, 2 ] and matches the equivalent RangeSelection
#[test]
fn arrow_right_with_1_0_selected_converts_to_0_2() {
    run_case(
        &KEY_ARROW_RIGHT_COMMAND,
        "ArrowRight",
        &[1, 0],
        (0, 2),
        (0, 3),
        true,
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > does not convert a discontiguous NodeSelection to a range
#[test]
fn does_not_convert_a_discontiguous_node_selection_to_a_range() {
    let editor = create_editor();
    select_indexes(&editor, &[0, 2]);
    let (default_prevented, handled) =
        dispatch_shift_arrow(&editor, &KEY_ARROW_RIGHT_COMMAND, "ArrowRight");
    assert!(handled);
    assert!(default_prevented);
    expect_root_range(&editor, 1, 1);
    editor.dispose();
}
