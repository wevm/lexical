//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts`.

#![cfg(feature = "extension")]

use super::support::{
    build_with_decorators, dispatch_ctrl_key, paragraph_with, select_nodes, set_root_children,
    snapshot_selection,
};
use crate::extension::LexicalEditorWithDispose;
use crate::rich_text::{HeadingTagType, create_heading_node};
use crate::upstream_tests::support::{create_test_decorator_node_inline, update};
use crate::{
    LexicalError, MOVE_TO_END, NodeRef, PointType, RangeSelection, create_text_node, get_root,
    get_selection,
};

fn inline_decorator() -> NodeRef {
    create_test_decorator_node_inline(true)
}

fn dispatch(editor: &LexicalEditorWithDispose, shift_key: bool) {
    dispatch_ctrl_key(editor, &MOVE_TO_END, "ArrowRight", shift_key);
}

fn range(editor: &LexicalEditorWithDispose) -> RangeSelection {
    editor
        .read(|| get_selection().and_then(|s| s.range()).expect("a range"))
        .unwrap()
}

fn first_key(editor: &LexicalEditorWithDispose) -> String {
    editor
        .read(|| get_root().get_first_child().unwrap().get_key().to_owned())
        .unwrap()
}

fn expect_no_op(setup: fn() -> Result<(), LexicalError>) {
    let editor = build_with_decorators(setup);
    let before = snapshot_selection(&editor);
    dispatch(&editor, false);
    assert_eq!(snapshot_selection(&editor), before);
    editor.dispose();
}

/// The `NodeSelection` case: `setup` in an update, then the command; the
/// selection collapses to element point `expected()`.
fn expect_node_selection_collapses(
    setup: fn() -> Result<(), LexicalError>,
    expected: fn() -> (String, usize),
) {
    let editor = build_with_decorators(|| Ok(()));
    update(&editor, setup);
    dispatch(&editor, false);
    let s = range(&editor);
    let (key, offset) = editor.read(expected).unwrap();
    assert!(s.is_collapsed());
    assert_eq!(s.anchor.point_type, PointType::Element);
    assert_eq!(s.anchor.key, key);
    assert_eq!(s.anchor.offset, offset);
    editor.dispose();
}

/// `[inline decorator][text "hello"]`, the caret at element offset 0.
fn leading_decorator_then_hello() -> Result<(), LexicalError> {
    let paragraph = paragraph_with(&[inline_decorator(), create_text_node("hello")])?;
    set_root_children(std::slice::from_ref(&paragraph))?;
    paragraph.select(Some(0), Some(0))?;
    Ok(())
}

/// A paragraph holding only an inline decorator, the caret at offset 0.
fn decorator_only() -> Result<(), LexicalError> {
    let paragraph = paragraph_with(&[inline_decorator()])?;
    set_root_children(std::slice::from_ref(&paragraph))?;
    paragraph.select(Some(0), Some(0))?;
    Ok(())
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a leading inline DecoratorNode (Issue #8555) > Cmd+ArrowRight at offset 0 moves caret past the inline decorator
#[test]
fn cmd_arrow_right_at_offset_0_moves_caret_past_the_inline_decorator() {
    let editor = build_with_decorators(leading_decorator_then_hello);
    dispatch(&editor, false);
    let s = range(&editor);
    assert!(s.is_collapsed());
    assert_eq!(s.focus.point_type, PointType::Text);
    assert_eq!(s.focus.offset, "hello".len());
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a leading inline DecoratorNode (Issue #8555) > Shift+Cmd+ArrowRight at offset 0 selects to end of element
#[test]
fn shift_cmd_arrow_right_at_offset_0_selects_to_end_of_element() {
    let editor = build_with_decorators(leading_decorator_then_hello);
    dispatch(&editor, true);
    let s = range(&editor);
    assert!(!s.is_collapsed());
    assert_eq!(s.anchor.point_type, PointType::Element);
    assert_eq!(s.anchor.offset, 0);
    assert_eq!(s.focus.offset, "hello".len());
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a leading inline DecoratorNode (Issue #8555) > Same fix applies inside HeadingNode
#[test]
fn same_fix_applies_inside_heading_node() {
    let editor = build_with_decorators(|| {
        let heading = create_heading_node(HeadingTagType::H1);
        heading.append(&[inline_decorator(), create_text_node("world")])?;
        set_root_children(std::slice::from_ref(&heading))?;
        heading.select(Some(0), Some(0))?;
        Ok(())
    });
    dispatch(&editor, false);
    let s = range(&editor);
    assert!(s.is_collapsed());
    assert_eq!(s.focus.offset, "world".len());
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END no-op cases (Issue #8555) > no-op: first child is text, not a decorator
#[test]
fn no_op_first_child_is_text_not_a_decorator() {
    expect_no_op(|| {
        let paragraph = paragraph_with(&[create_text_node("plain")])?;
        set_root_children(std::slice::from_ref(&paragraph))?;
        paragraph.select(Some(0), Some(0))?;
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END no-op cases (Issue #8555) > no-op: first child is a block (non-inline) dec…
#[test]
fn no_op_first_child_is_a_block_decorator() {
    expect_no_op(|| {
        let paragraph = paragraph_with(&[
            create_test_decorator_node_inline(false),
            create_text_node("after"),
        ])?;
        set_root_children(std::slice::from_ref(&paragraph))?;
        paragraph.select(Some(0), Some(0))?;
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END no-op cases (Issue #8555) > no-op: caret is past offset 0
#[test]
fn no_op_caret_is_past_offset_0() {
    expect_no_op(|| {
        let paragraph = paragraph_with(&[inline_decorator(), create_text_node("hello")])?;
        set_root_children(std::slice::from_ref(&paragraph))?;
        paragraph.select(Some(1), Some(1))?;
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END with no trailing text (Issue #8601) > Cmd+ArrowRight on decorator-only element moves caret past the last child
#[test]
fn cmd_arrow_right_on_decorator_only_element_moves_caret_past_the_last_child() {
    let editor = build_with_decorators(decorator_only);
    dispatch(&editor, false);
    let s = range(&editor);
    let paragraph = first_key(&editor);
    assert!(s.is_collapsed());
    assert_eq!(s.focus.point_type, PointType::Element);
    assert_eq!(s.focus.key, paragraph);
    assert_eq!(s.focus.offset, 1);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END with no trailing text (Issue #8601) > Shift+Cmd+ArrowRight on decorator-only element selects past the last child
#[test]
fn shift_cmd_arrow_right_on_decorator_only_element_selects_past_the_last_child() {
    let editor = build_with_decorators(decorator_only);
    dispatch(&editor, true);
    let s = range(&editor);
    let paragraph = first_key(&editor);
    assert!(!s.is_collapsed());
    assert_eq!(s.anchor.point_type, PointType::Element);
    assert_eq!(s.anchor.key, paragraph);
    assert_eq!(s.anchor.offset, 0);
    assert_eq!(s.focus.point_type, PointType::Element);
    assert_eq!(s.focus.key, paragraph);
    assert_eq!(s.focus.offset, 1);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END with no trailing text (Issue #8601) > Cmd+ArrowRight on [decorator][text][decorator] sandwich moves caret past the trailing decorator
#[test]
fn cmd_arrow_right_on_sandwich_moves_caret_past_the_trailing_decorator() {
    let editor = build_with_decorators(|| {
        let paragraph = paragraph_with(&[
            inline_decorator(),
            create_text_node("hello"),
            inline_decorator(),
        ])?;
        set_root_children(std::slice::from_ref(&paragraph))?;
        paragraph.select(Some(0), Some(0))?;
        Ok(())
    });
    dispatch(&editor, false);
    let s = range(&editor);
    let paragraph = first_key(&editor);
    assert!(s.is_collapsed());
    assert_eq!(s.focus.point_type, PointType::Element);
    assert_eq!(s.focus.key, paragraph);
    assert_eq!(s.focus.offset, 3);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a NodeSelection (Issue #8604) > single inline decorator collapses to th…
#[test]
fn single_inline_decorator_collapses_to_the_block_end() {
    expect_node_selection_collapses(
        || {
            let decorator = inline_decorator();
            set_root_children(&[paragraph_with(&[
                decorator.clone(),
                create_text_node("hello"),
            ])?])?;
            select_nodes(&[decorator]);
            Ok(())
        },
        || {
            (
                get_root().get_first_child().unwrap().get_key().to_owned(),
                2,
            )
        },
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a NodeSelection (Issue #8604) > multi-node selection picks the iteratio…
#[test]
fn multi_node_selection_picks_the_iteration_order_last_node() {
    expect_node_selection_collapses(
        || {
            let (first, second) = (inline_decorator(), inline_decorator());
            set_root_children(&[
                paragraph_with(&[first.clone(), create_text_node("A")])?,
                paragraph_with(&[second.clone(), create_text_node("B")])?,
            ])?;
            select_nodes(&[first, second]);
            Ok(())
        },
        || {
            (
                get_root()
                    .get_child_at_index(1)
                    .unwrap()
                    .get_key()
                    .to_owned(),
                2,
            )
        },
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a NodeSelection (Issue #8604) > whole-element non-inline ElementNode re…
#[test]
fn whole_element_non_inline_element_node_resolves_to_its_parent() {
    // The paragraph itself is skipped, so the lookup falls back to root.
    expect_node_selection_collapses(
        || {
            let paragraph = paragraph_with(&[create_text_node("hello")])?;
            set_root_children(std::slice::from_ref(&paragraph))?;
            select_nodes(&[paragraph]);
            Ok(())
        },
        || ("root".to_owned(), get_root().get_children_size()),
    );
}
