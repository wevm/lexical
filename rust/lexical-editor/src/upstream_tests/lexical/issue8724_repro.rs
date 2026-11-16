//! Ports of `packages/lexical/src/__tests__/unit/Issue8724Repro.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::{
    TestBlockableDecoratorNode, TestElementNode, TestShadowRootNode,
    create_test_decorator_node_inline, create_test_element_node, create_test_shadow_root_node,
    range_selection,
};
use crate::NodeClass;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::{
    AnyLexicalExtensionArgument, LexicalError, LexicalExtension, NodeRef, UpdateOptions,
    create_paragraph_node, create_text_node, define_extension, get_node_by_key, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

fn build(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
    nodes: Vec<NodeClass>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `ext`.
fn ext() -> LexicalEditorWithDispose {
    build(
        "[8724]",
        vec![crate::rich_text::rich_text_extension().into()],
        vec![
            NodeClass::of::<TestBlockableDecoratorNode>(),
            NodeClass::of::<TestElementNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
    )
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn with(node: NodeRef, children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    node.append(children)?;
    Ok(node)
}

fn set_root(children: &[NodeRef]) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(children)
}

/// Inserts a new block decorator at the selection and returns its key.
fn insert_block_decorator(editor: &LexicalEditorWithDispose) -> String {
    let key = Rc::new(RefCell::new(String::new()));
    let k = key.clone();
    editor
        .update(
            move || {
                let inserted = create_test_decorator_node_inline(false);
                *k.borrow_mut() = inserted.get_key().to_owned();
                range_selection().insert_nodes(vec![inserted])
            },
            discrete(),
        )
        .unwrap();
    key.take()
}

fn parent_type_of(editor: &LexicalEditorWithDispose, key: String) -> String {
    editor
        .read(move || {
            let inserted = get_node_by_key(&key).expect("Expected the inserted node to exist");
            inserted.get_parent().unwrap().get_type().to_owned()
        })
        .unwrap()
}

// upstream: lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization of non-inline nodes (#8713, #8724) > block node at a block cursor inside an inline-only element splits up to the shadow root
#[test]
fn block_node_at_a_block_cursor_inside_an_inline_only_element_splits_up_to_the_shadow_root() {
    let editor = ext();
    editor
        .update(
            || {
                let element = with(
                    create_test_element_node(),
                    &[create_test_decorator_node_inline(false)],
                )?;
                let shadow = with(
                    create_test_shadow_root_node(),
                    std::slice::from_ref(&element),
                )?;
                set_root(&[shadow])?;
                element.select(Some(1), Some(1))?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let key = insert_block_decorator(&editor);
    assert_eq!(parent_type_of(&editor, key), "test_shadow_root");
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization of non-inline nodes (#8713, #8724) > block node at a block cursor inside an inline-only element splits up to the root
#[test]
fn block_node_at_a_block_cursor_inside_an_inline_only_element_splits_up_to_the_root() {
    let editor = ext();
    editor
        .update(
            || {
                let element = with(
                    create_test_element_node(),
                    &[create_test_decorator_node_inline(false)],
                )?;
                set_root(std::slice::from_ref(&element))?;
                element.select(Some(1), Some(1))?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let key = insert_block_decorator(&editor);
    assert_eq!(parent_type_of(&editor, key), "root");
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization of non-inline nodes (#8713, #8724) > still inserts a block node directly at a block cursor on a shadow root (#8708)
#[test]
fn still_inserts_a_block_node_directly_at_a_block_cursor_on_a_shadow_root() {
    let editor = ext();
    editor
        .update(
            || {
                let shadow = with(
                    create_test_shadow_root_node(),
                    &[create_test_decorator_node_inline(false)],
                )?;
                set_root(std::slice::from_ref(&shadow))?;
                shadow.select(Some(1), Some(1))?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    insert_block_decorator(&editor);
    editor
        .read(|| {
            let shadow = get_root().get_first_child().unwrap();
            assert_eq!(shadow.get_type(), "test_shadow_root");
            assert_eq!(shadow.get_children_size(), 2);
        })
        .unwrap();
    editor.dispose();
}

#[cfg(feature = "list")]
mod lists {
    use super::*;
    use crate::list::{
        ListType, create_list_item_node, create_list_node, is_list_item_node, is_list_node,
    };

    /// `listExt`.
    fn list_ext() -> LexicalEditorWithDispose {
        build(
            "[8724-list]",
            vec![
                crate::rich_text::rich_text_extension().into(),
                crate::list::list_extension().into(),
            ],
            vec![NodeClass::of::<TestBlockableDecoratorNode>()],
        )
    }

    /// `$assertNoBlockInsideListItem(node)`.
    fn assert_no_block_inside_list_item(node: &NodeRef) {
        if is_list_item_node(node) {
            for child in node.get_children() {
                assert_eq!(
                    (child.is_element() || child.is_decorator()) && !child.is_inline(),
                    is_list_node(&child),
                    "ListItemNode should not directly hold non-inline {}",
                    child.get_type()
                );
            }
        }
        if node.is_element() {
            for child in node.get_children() {
                assert_no_block_inside_list_item(&child);
            }
        }
    }

    fn expect_not_in_list_item(editor: &LexicalEditorWithDispose, key: String) {
        editor
            .read(move || {
                let inserted = get_node_by_key(&key).expect("Expected the inserted node to exist");
                assert!(!is_list_item_node(&inserted.get_parent().unwrap()));
                assert_no_block_inside_list_item(&get_root());
            })
            .unwrap();
    }

    // upstream: lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization does not regress lists (#8724) > block node pasted in a list item text splits the list (unchanged CASE 3)
    #[test]
    fn block_node_pasted_in_a_list_item_text_splits_the_list() {
        let editor = list_ext();
        editor
            .update(
                || {
                    let text = create_text_node("hello");
                    let item = with(create_list_item_node(None), std::slice::from_ref(&text))?;
                    let list = with(create_list_node(ListType::Bullet, 1.0), &[item])?;
                    set_root(&[list])?;
                    text.select(Some(2), Some(2))?;
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
        let key = insert_block_decorator(&editor);
        expect_not_in_list_item(&editor, key);
        editor.dispose();
    }

    // upstream: lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization does not regress lists (#8724) > block node at an element point on a list item holding a nested list does not crash
    #[test]
    fn block_node_at_an_element_point_on_a_list_item_holding_a_nested_list_does_not_crash() {
        let editor = list_ext();
        editor
            .update(
                || {
                    let first_item = with(create_list_item_node(None), &[create_text_node("a")])?;
                    let nested_item = with(create_list_item_node(None), &[create_text_node("b")])?;
                    let nested_list =
                        with(create_list_node(ListType::Bullet, 1.0), &[nested_item])?;
                    let nesting_item = with(create_list_item_node(None), &[nested_list])?;
                    let list = with(
                        create_list_node(ListType::Bullet, 1.0),
                        &[first_item, nesting_item.clone()],
                    )?;
                    set_root(&[list])?;
                    nesting_item.select(Some(0), Some(0))?;
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
        let key = insert_block_decorator(&editor);
        expect_not_in_list_item(&editor, key);
        editor.dispose();
    }
}

/// A test element holding a paragraph `abc`, the caret at its end.
fn element_with_paragraph(editor: &LexicalEditorWithDispose) {
    editor
        .update(
            || {
                let text = create_text_node("abc");
                let paragraph = with(create_paragraph_node(), std::slice::from_ref(&text))?;
                let element = with(create_test_element_node(), &[paragraph])?;
                set_root(&[element])?;
                text.select(Some(3), Some(3))?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

fn first_child_text(editor: &LexicalEditorWithDispose) -> String {
    editor
        .read(|| {
            let element = get_root().get_first_child().unwrap();
            assert!(element.is_element());
            element.get_text_content()
        })
        .unwrap()
}

// upstream: lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste flattens non-inline nodes inside an inline-only element (#8724) > drops a block-only paste with no inline form
#[test]
fn drops_a_block_only_paste_with_no_inline_form() {
    let editor = ext();
    element_with_paragraph(&editor);
    let key = insert_block_decorator(&editor);
    assert!(
        editor
            .read(move || get_node_by_key(&key).is_none())
            .unwrap()
    );
    assert_eq!(first_child_text(&editor), "abc");
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste flattens non-inline nodes inside an inline-only element (#8724) > keeps the inline content of a block paste
#[test]
fn keeps_the_inline_content_of_a_block_paste() {
    let editor = ext();
    element_with_paragraph(&editor);
    editor
        .update(
            || {
                range_selection().insert_nodes(vec![with(
                    create_paragraph_node(),
                    &[create_text_node("def")],
                )?])
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(first_child_text(&editor), "abcdef");
    editor.dispose();
}
