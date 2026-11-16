//! Ports of `packages/lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::upstream_tests::support::{
    TestBlockableDecoratorNode, TestShadowRootNode, create_test_blockable_decorator_node,
    create_test_shadow_root_node, set_is_inline, update,
};
use crate::{
    LexicalExtension, NodeClass, NodeRef, create_paragraph_node, create_text_node,
    define_extension, get_root,
};

fn build() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        nodes: vec![
            NodeClass::of::<TestBlockableDecoratorNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

fn is_paragraph(node: &NodeRef) -> bool {
    node.get_type() == "paragraph"
}

/// Expects `parent` to hold one paragraph whose text is `x`.
fn expect_one_paragraph_x(parent: &NodeRef) {
    let children = parent.get_children();
    assert_eq!(children.len(), 1);
    assert!(is_paragraph(&children[0]), "Expected ParagraphNode");
    assert_eq!(children[0].get_text_content(), "x");
}

// upstream: lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts::Typing at root + last-offset selection (no phantom paragraph) > reuses an empty trailing paragraph instead of appending a new one
#[test]
fn reuses_an_empty_trailing_paragraph_instead_of_appending_a_new_one() {
    let editor = build();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[create_paragraph_node()])?;
        root.select(Some(1), Some(1))?.insert_text("x")
    });
    editor.read(|| expect_one_paragraph_x(&get_root())).unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts::Typing at root + last-offset selection (no phantom paragraph) > empty root still creates a new paragraph for the typed text
#[test]
fn empty_root_still_creates_a_new_paragraph_for_the_typed_text() {
    let editor = build();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.select(Some(0), Some(0))?.insert_text("x")
    });
    editor.read(|| expect_one_paragraph_x(&get_root())).unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts::Typing at root + last-offset selection (no phantom paragraph) > reuses an empty trailing paragraph inside a shadow root
#[test]
fn reuses_an_empty_trailing_paragraph_inside_a_shadow_root() {
    let editor = build();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let shadow_root = create_test_shadow_root_node();
        shadow_root.append(&[create_paragraph_node()])?;
        root.append(std::slice::from_ref(&shadow_root))?;
        shadow_root.select(Some(1), Some(1))?.insert_text("x")
    });
    editor
        .read(|| {
            let shadow_root = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(
                shadow_root.get_type(),
                "test_shadow_root",
                "Expected TestShadowRootNode"
            );
            expect_one_paragraph_x(&shadow_root);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts::Typing at root + last-offset selection (no phantom paragraph) > falls back to a new paragraph when the trailing child is a DecoratorNode
#[test]
fn falls_back_to_a_new_paragraph_when_the_trailing_child_is_a_decorator_node() {
    let editor = build();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("first")])?;
        root.append(&[
            paragraph,
            set_is_inline(&create_test_blockable_decorator_node(), false),
        ])?;
        root.select(Some(2), Some(2))?.insert_text("x")
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 3);
            assert!(is_paragraph(&children[0]), "Expected first paragraph");
            assert_eq!(children[0].get_text_content(), "first");
            assert_eq!(children[1].get_type(), "test_decorator");
            assert!(is_paragraph(&children[2]), "Expected trailing paragraph");
            assert_eq!(children[2].get_text_content(), "x");
        })
        .unwrap();
    editor.dispose();
}
