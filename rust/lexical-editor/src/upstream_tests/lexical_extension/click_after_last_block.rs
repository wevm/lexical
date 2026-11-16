//! Ports of `packages/lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts`.
//!
//! JS dispatches `mousedown` and `click` on the root element below the
//! last block. The Rust extension follows the view, which reports such a
//! click as `CLICK_COMMAND` with `root` as its `target_key` (see the
//! extension's docs), so each port reports that click.

#![cfg(feature = "rich-text")]

use super::super::support::{
    TestBlockableDecoratorNode, create_test_blockable_decorator_node, set_is_inline,
    set_test_root_element,
};
use super::support::paragraph;
use crate::extension::{
    ClickAfterLastBlockConfigOverrides, LexicalEditorWithDispose, ShouldInsertAfter,
    build_editor_from_extensions, click_after_last_block_extension,
};
use crate::{
    CLICK_COMMAND, Editor, InitialEditorStateType, LexicalExtension, MouseEvent, NodeClass,
    config_extension, define_extension, get_root,
};
use std::rc::Rc;

/// `setUpEditor({disabled, $shouldInsertAfter, withInitialChildren})`.
fn set_up_editor(
    disabled: bool,
    should_insert_after: Option<ShouldInsertAfter>,
    with_initial_children: bool,
) -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
            if with_initial_children {
                // A block-level decorator at the end of the document.
                let decorator = set_is_inline(&create_test_blockable_decorator_node(), false);
                get_root().append(&[paragraph("hello"), decorator])?;
            }
            Ok(())
        }))),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            config_extension(
                &click_after_last_block_extension(),
                [ClickAfterLastBlockConfigOverrides {
                    disabled: Some(disabled),
                    should_insert_after,
                }],
            ),
        ],
        nodes: vec![NodeClass::of::<TestBlockableDecoratorNode>()],
        ..LexicalExtension::<(), (), ()>::new("click-after-last-block-test", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

/// The click below the last block, on the root.
fn click_below(editor: &Editor) {
    editor
        .dispatch_command(
            &CLICK_COMMAND,
            Rc::new(MouseEvent {
                client_y: 100.0,
                target_key: Some("root".to_owned()),
                ..MouseEvent::default()
            }),
        )
        .unwrap();
}

fn children_size(editor: &Editor) -> usize {
    editor.read(|| get_root().get_children_size()).unwrap()
}

// upstream: lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > inserts a paragraph after the last block when the last child is a block decorator
#[test]
fn inserts_a_paragraph_after_a_block_decorator() {
    let editor = set_up_editor(false, None, true);
    assert_eq!(children_size(&editor), 2);
    click_below(&editor);
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 3);
            let last = root.get_last_child().unwrap();
            assert_eq!(last.get_type(), "paragraph");
            assert!(last.is_empty());
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > does nothing when the predicate rejects the last child
#[test]
fn does_nothing_when_the_predicate_rejects() {
    let editor = set_up_editor(false, Some(ShouldInsertAfter(Rc::new(|_| false))), true);
    click_below(&editor);
    assert_eq!(children_size(&editor), 2);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > respects disabled config
#[test]
fn respects_disabled_config() {
    let editor = set_up_editor(true, None, true);
    click_below(&editor);
    assert_eq!(children_size(&editor), 2);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > does nothing in read-only mode
#[test]
fn does_nothing_in_read_only_mode() {
    let editor = set_up_editor(false, None, true);
    editor.set_editable(false);
    click_below(&editor);
    assert_eq!(children_size(&editor), 2);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > does nothing on an empty root with no last child
#[test]
fn does_nothing_on_an_empty_root() {
    let editor = set_up_editor(false, None, false);
    click_below(&editor);
    assert_eq!(children_size(&editor), 0);
    editor.dispose();
}
