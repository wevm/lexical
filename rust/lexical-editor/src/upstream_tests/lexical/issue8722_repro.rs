//! Ports of `packages/lexical/src/__tests__/unit/Issue8722Repro.test.ts`.
//!
//! Upstream's test editor has jsdom's window, installed after Lexical
//! loaded, so `deleteLine`'s `modify` has no DOM selection and stops after
//! its model step; [`NoDomSelection`] says so here.

#![cfg(all(feature = "extension", feature = "rich-text", feature = "list"))]

use super::super::support::{
    TestBlockableDecoratorNode, create_test_decorator_node_inline, range_selection,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::{
    LexicalError, LexicalExtension, NodeClass, Point, SelectionGranularity, SelectionMovement,
    UpdateOptions, create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::rc::Rc;

struct NoDomSelection;

impl SelectionMovement for NoDomSelection {
    fn landed_point(
        &self,
        _from: &Point,
        _is_backward: bool,
        _granularity: SelectionGranularity,
    ) -> Result<Option<Point>, LexicalError> {
        Ok(None)
    }
    fn has_dom_selection(&self) -> bool {
        false
    }
}

fn build() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
        ],
        nodes: vec![NodeClass::of::<TestBlockableDecoratorNode>()],
        ..LexicalExtension::<(), (), ()>::new("[8722]", ())
    })
    .into()])
    .unwrap();
    editor.set_selection_movement(Some(Rc::new(NoDomSelection)));
    editor
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn run(
    setup: impl FnOnce() -> Result<(), LexicalError> + 'static,
    delete: impl FnOnce() -> Result<(), LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    let editor = build();
    editor.update(setup, discrete()).unwrap();
    editor.update(delete, discrete()).unwrap();
    editor
}

fn has_decorator(editor: &LexicalEditorWithDispose) -> bool {
    editor
        .read(|| get_root().get_children().iter().any(|n| n.is_decorator()))
        .unwrap()
}

/// A block decorator, then a bullet list with one empty item, the caret in
/// it (or the other way round when `list_first`).
fn decorator_and_empty_item(list_first: bool) -> Result<(), LexicalError> {
    let decorator = create_test_decorator_node_inline(false);
    let list = create_list_node(ListType::Bullet, 1.0);
    let item = create_list_item_node(None);
    list.append(std::slice::from_ref(&item))?;
    let root = get_root();
    root.clear()?;
    if list_first {
        root.append(&[list, decorator])?;
    } else {
        root.append(&[decorator, list])?;
    }
    item.select(Some(0), Some(0))?;
    Ok(())
}

// upstream: lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteCharacter backward from empty ListItem should not remove the preceding decorator
#[test]
fn delete_character_backward_from_empty_list_item_should_not_remove_the_preceding_decorator() {
    let editor = run(
        || decorator_and_empty_item(false),
        || range_selection().delete_character(true),
    );
    assert!(has_decorator(&editor));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine backward from empty ListItem with prece… should not remove the decorator
#[test]
fn delete_line_backward_from_empty_list_item_with_preceding_decorator() {
    let editor = run(
        || decorator_and_empty_item(false),
        || range_selection().delete_line(true),
    );
    assert!(has_decorator(&editor));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine backward from empty paragraph with prec… should not remove the decorator
#[test]
fn delete_line_backward_from_empty_paragraph_with_preceding_decorator() {
    let editor = run(
        || {
            let decorator = create_test_decorator_node_inline(false);
            let paragraph = create_paragraph_node();
            let root = get_root();
            root.clear()?;
            root.append(&[decorator, paragraph.clone()])?;
            paragraph.select(Some(0), Some(0))?;
            Ok(())
        },
        || range_selection().delete_line(true),
    );
    assert!(has_decorator(&editor));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine forward from empty ListItem with follow… should not remove the decorator
#[test]
fn delete_line_forward_from_empty_list_item_with_following_decorator() {
    let editor = run(
        || decorator_and_empty_item(true),
        || range_selection().delete_line(false),
    );
    assert!(has_decorator(&editor));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine backward from empty ListItem preceded by empty paragraph merges them
#[test]
fn delete_line_backward_from_empty_list_item_preceded_by_empty_paragraph_merges_them() {
    let editor = run(
        || {
            let paragraph = create_paragraph_node();
            let list = create_list_node(ListType::Bullet, 1.0);
            let item = create_list_item_node(None);
            list.append(std::slice::from_ref(&item))?;
            let root = get_root();
            root.clear()?;
            root.append(&[paragraph, list])?;
            item.select(Some(0), Some(0))?;
            Ok(())
        },
        || range_selection().delete_line(true),
    );
    assert_eq!(editor.read(|| get_root().get_children_size()).unwrap(), 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine backward from empty ListItem preceded by non-empty paragraph should work normally
#[test]
fn delete_line_backward_from_empty_list_item_preceded_by_non_empty_paragraph() {
    let editor = run(
        || {
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("hello")])?;
            let list = create_list_node(ListType::Bullet, 1.0);
            let item = create_list_item_node(None);
            list.append(std::slice::from_ref(&item))?;
            let root = get_root();
            root.clear()?;
            root.append(&[paragraph, list])?;
            item.select(Some(0), Some(0))?;
            Ok(())
        },
        || range_selection().delete_line(true),
    );
    assert_eq!(
        editor.read(|| get_root().get_text_content()).unwrap(),
        "hello"
    );
    editor.dispose();
}
