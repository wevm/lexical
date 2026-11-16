//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts`.
//! The three `$updateDOMBlockCursorElement` tests look for the block cursor
//! element in the DOM and are skipped (dom-only).

#![cfg(feature = "extension")]

use super::support::{key_event, select_nodes};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::rich_text::rich_text_extension;
use crate::upstream_tests::support::{
    TestBlockableDecoratorNode, TestShadowRootNode, create_test_decorator_node_inline,
    create_test_shadow_root_node, update,
};
use crate::{
    InitialEditorStateType, KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND,
    KEY_ARROW_RIGHT_COMMAND, KEY_ARROW_UP_COMMAND, KeyboardEvent, LexicalCommand, LexicalExtension,
    NodeClass, NodeRef, PointType, create_paragraph_node, create_text_node, define_extension,
    get_root, get_selection,
};
use std::rc::Rc;

fn build(
    initial: impl Fn() -> Result<(), crate::LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        dependencies: vec![rich_text_extension().into()],
        nodes: vec![
            NodeClass::of::<TestBlockableDecoratorNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("test", ())
    })
    .into()])
    .unwrap()
}

fn paragraph_of(text: &str) -> Result<NodeRef, crate::LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)])?;
    Ok(paragraph)
}

fn shadow_root_of(children: &[NodeRef]) -> Result<NodeRef, crate::LexicalError> {
    let shadow = create_test_shadow_root_node();
    shadow.append(children)?;
    Ok(shadow)
}

fn set_root(children: &[NodeRef]) -> Result<(), crate::LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(children)
}

// Structure used across tests:
//   root
//     decorator (block)       — index 0
//     shadowRoot              — index 1
//       paragraph > text
//     decorator (block)       — index 2
fn create_decorator_shadow_root_editor() -> LexicalEditorWithDispose {
    build(|| {
        set_root(&[
            create_test_decorator_node_inline(false),
            shadow_root_of(&[paragraph_of("inside")?])?,
            create_test_decorator_node_inline(false),
        ])
    })
}

/// The same with the paragraph in an inner shadow root of the outer one.
fn create_nested_editor() -> LexicalEditorWithDispose {
    build(|| {
        set_root(&[
            create_test_decorator_node_inline(false),
            shadow_root_of(&[shadow_root_of(&[paragraph_of("nested")?])?])?,
            create_test_decorator_node_inline(false),
        ])
    })
}

fn press(
    editor: &LexicalEditorWithDispose,
    command: &LexicalCommand<Rc<KeyboardEvent>>,
    key: &str,
) {
    editor
        .dispatch_command(command, key_event(key, false))
        .unwrap();
}

fn select_child(editor: &LexicalEditorWithDispose, index: usize) {
    update(editor, move || {
        select_nodes(&[get_root().get_child_at_index(index).unwrap()]);
        Ok(())
    });
}

fn select_root(editor: &LexicalEditorWithDispose, offset: usize) {
    update(editor, move || {
        get_root().select(Some(offset), Some(offset)).map(drop)
    });
}

/// The text inside the shadow root at root index 1 (through `depth`
/// shadow roots).
fn inner_text(depth: usize) -> NodeRef {
    let mut node = get_root().get_child_at_index(1).unwrap();
    for _ in 1..depth {
        node = node.get_first_child().unwrap();
    }
    node.get_first_child().unwrap().get_first_child().unwrap()
}

/// Selects offset `offset` of the paragraph (`None`) or of its text
/// (`Some`), inside the shadow root(s) at root index 1.
fn select_inside(editor: &LexicalEditorWithDispose, depth: usize, text_offset: Option<usize>) {
    update(editor, move || {
        let text = inner_text(depth);
        match text_offset {
            Some(offset) => text.select(Some(offset), Some(offset))?,
            None => text.get_parent().unwrap().select(Some(0), Some(0))?,
        };
        Ok(())
    });
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

fn expect_in_text_at(editor: &LexicalEditorWithDispose, offset: usize) {
    editor
        .read(|| {
            let s = get_selection().and_then(|s| s.range()).expect("a range");
            assert!(s.is_collapsed());
            assert_eq!(s.anchor.key, inner_text(1).get_key());
            assert_eq!(s.anchor.offset, offset);
        })
        .unwrap();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$exitNodeSelectionToward — decorator → block cursor beside shadow root (#8736) > ArrowDown from decorator[0] lands at block cursor (offset 1)
#[test]
fn arrow_down_from_decorator_0_lands_at_block_cursor_offset_1() {
    let editor = create_decorator_shadow_root_editor();
    select_child(&editor, 0);
    press(&editor, &KEY_ARROW_DOWN_COMMAND, "ArrowDown");
    expect_block_cursor_at(&editor, 1);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$exitNodeSelectionToward — decorator → block cursor beside shadow root (#8736) > ArrowRight from decorator[0] lands at block cursor (offset 1)
#[test]
fn arrow_right_from_decorator_0_lands_at_block_cursor_offset_1() {
    let editor = create_decorator_shadow_root_editor();
    select_child(&editor, 0);
    press(&editor, &KEY_ARROW_RIGHT_COMMAND, "ArrowRight");
    expect_block_cursor_at(&editor, 1);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$exitNodeSelectionToward — decorator → block cursor beside shadow root (#8736) > ArrowUp from decorator[2] lands at block cursor (offset 2)
#[test]
fn arrow_up_from_decorator_2_lands_at_block_cursor_offset_2() {
    let editor = create_decorator_shadow_root_editor();
    select_child(&editor, 2);
    press(&editor, &KEY_ARROW_UP_COMMAND, "ArrowUp");
    expect_block_cursor_at(&editor, 2);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$exitNodeSelectionToward — decorator → block cursor beside shadow root (#8736) > ArrowLeft from decorator[2] lands at block cursor (offset 2)
#[test]
fn arrow_left_from_decorator_2_lands_at_block_cursor_offset_2() {
    let editor = create_decorator_shadow_root_editor();
    select_child(&editor, 2);
    press(&editor, &KEY_ARROW_LEFT_COMMAND, "ArrowLeft");
    expect_block_cursor_at(&editor, 2);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryEnterFromBlockCursor — block cursor → enter shadow root (#8736) > ArrowRight from block cursor before shadow root enters at selectStart
#[test]
fn arrow_right_from_block_cursor_before_shadow_root_enters_at_selectstart() {
    let editor = create_decorator_shadow_root_editor();
    select_root(&editor, 1);
    press(&editor, &KEY_ARROW_RIGHT_COMMAND, "ArrowRight");
    // selectStart() and selectEnd() resolve to the leaf text node
    expect_in_text_at(&editor, 0);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryEnterFromBlockCursor — block cursor → enter shadow root (#8736) > ArrowDown from block cursor before shadow root enters at selectStart
#[test]
fn arrow_down_from_block_cursor_before_shadow_root_enters_at_selectstart() {
    let editor = create_decorator_shadow_root_editor();
    select_root(&editor, 1);
    press(&editor, &KEY_ARROW_DOWN_COMMAND, "ArrowDown");
    // selectStart() and selectEnd() resolve to the leaf text node
    expect_in_text_at(&editor, 0);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryEnterFromBlockCursor — block cursor → enter shadow root (#8736) > ArrowUp from block cursor after shadow root enters at selectEnd
#[test]
fn arrow_up_from_block_cursor_after_shadow_root_enters_at_selectend() {
    let editor = create_decorator_shadow_root_editor();
    select_root(&editor, 2);
    press(&editor, &KEY_ARROW_UP_COMMAND, "ArrowUp");
    // selectStart() and selectEnd() resolve to the leaf text node
    expect_in_text_at(&editor, 6);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryEnterFromBlockCursor — block cursor → enter shadow root (#8736) > ArrowLeft from block cursor after shadow root enters at selectEnd
#[test]
fn arrow_left_from_block_cursor_after_shadow_root_enters_at_selectend() {
    let editor = create_decorator_shadow_root_editor();
    select_root(&editor, 2);
    press(&editor, &KEY_ARROW_LEFT_COMMAND, "ArrowLeft");
    // selectStart() and selectEnd() resolve to the leaf text node
    expect_in_text_at(&editor, 6);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryExitShadowRootToBlockCursor — shadow root → block cursor (#8736) > ArrowLeft at start of shadow root exits to block cursor before it
#[test]
fn arrow_left_at_start_of_shadow_root_exits_to_block_cursor_before_it() {
    let editor = create_decorator_shadow_root_editor();
    select_inside(&editor, 1, None);
    press(&editor, &KEY_ARROW_LEFT_COMMAND, "ArrowLeft");
    expect_block_cursor_at(&editor, 1);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryExitShadowRootToBlockCursor — shadow root → block cursor (#8736) > ArrowUp at start of shadow root exits to block cursor before it
#[test]
fn arrow_up_at_start_of_shadow_root_exits_to_block_cursor_before_it() {
    let editor = create_decorator_shadow_root_editor();
    select_inside(&editor, 1, None);
    press(&editor, &KEY_ARROW_UP_COMMAND, "ArrowUp");
    expect_block_cursor_at(&editor, 1);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryExitShadowRootToBlockCursor — shadow root → block cursor (#8736) > ArrowDown at end of shadow root exits to block cursor after it
#[test]
fn arrow_down_at_end_of_shadow_root_exits_to_block_cursor_after_it() {
    let editor = create_decorator_shadow_root_editor();
    select_inside(&editor, 1, Some(6));
    press(&editor, &KEY_ARROW_DOWN_COMMAND, "ArrowDown");
    expect_block_cursor_at(&editor, 2);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryExitShadowRootToBlockCursor — shadow root → block cursor (#8736) > ArrowRight at end of shadow root exits to block cursor after it
#[test]
fn arrow_right_at_end_of_shadow_root_exits_to_block_cursor_after_it() {
    let editor = create_decorator_shadow_root_editor();
    select_inside(&editor, 1, Some(6));
    press(&editor, &KEY_ARROW_RIGHT_COMMAND, "ArrowRight");
    expect_block_cursor_at(&editor, 2);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::nested shadow root exit (#8736) > ArrowLeft at start of inner shadow root walks up and exits to block cursor
#[test]
fn arrow_left_at_start_of_inner_shadow_root_walks_up_and_exits_to_block_cursor() {
    let editor = create_nested_editor();
    select_inside(&editor, 2, None);
    press(&editor, &KEY_ARROW_LEFT_COMMAND, "ArrowLeft");
    expect_block_cursor_at(&editor, 1);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::nested shadow root exit (#8736) > ArrowRight at end of inner shadow root walks up and exits to block cursor
#[test]
fn arrow_right_at_end_of_inner_shadow_root_walks_up_and_exits_to_block_cursor() {
    let editor = create_nested_editor();
    select_inside(&editor, 2, Some(6));
    press(&editor, &KEY_ARROW_RIGHT_COMMAND, "ArrowRight");
    expect_block_cursor_at(&editor, 2);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::no block cursor between sibling shadow roots inside a shadow root parent (#8736) > ArrowLeft at start of sibling shadow root does not produce block cursor
#[test]
fn arrow_left_at_start_of_sibling_shadow_root_does_not_produce_block_cursor() {
    // root > outerShadowRoot > (shadowRoot1, shadowRoot2), as a layout
    // container's items.
    let editor = build(|| {
        set_root(&[shadow_root_of(&[
            shadow_root_of(&[paragraph_of("col1")?])?,
            shadow_root_of(&[paragraph_of("col2")?])?,
        ])?])
    });
    update(&editor, || {
        let outer = get_root().get_first_child().unwrap();
        let sr2 = outer.get_child_at_index(1).unwrap();
        sr2.get_first_child().unwrap().select(Some(0), Some(0))?;
        Ok(())
    });
    press(&editor, &KEY_ARROW_LEFT_COMMAND, "ArrowLeft");
    // No block cursor on the outer shadow root.
    editor
        .read(|| {
            let outer_key = get_root().get_first_child().unwrap().get_key().to_owned();
            let s = get_selection().and_then(|s| s.range()).expect("a range");
            assert_ne!(s.anchor.key, outer_key);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::full round-trip: decorator → block cursor → shadow root → block cursor → decorator (#8736) > ArrowRight traversal: decorator → enter shadow root → exit → reach next decorator
#[test]
fn arrow_right_traversal_decorator_enter_shadow_root_exit_reach_next_decorator() {
    let editor = create_decorator_shadow_root_editor();
    select_child(&editor, 0);
    // Step 1: ArrowRight → block cursor at root:1
    press(&editor, &KEY_ARROW_RIGHT_COMMAND, "ArrowRight");
    expect_block_cursor_at(&editor, 1);
    // Step 2: ArrowRight → enter shadow root (selectStart → text:0)
    press(&editor, &KEY_ARROW_RIGHT_COMMAND, "ArrowRight");
    expect_in_text_at(&editor, 0);
    // Step 3: place the cursor at the end of the text
    select_inside(&editor, 1, Some(6));
    // Step 4: ArrowRight → exit shadow root → block cursor at root:2
    press(&editor, &KEY_ARROW_RIGHT_COMMAND, "ArrowRight");
    expect_block_cursor_at(&editor, 2);
    editor.dispose();
}
