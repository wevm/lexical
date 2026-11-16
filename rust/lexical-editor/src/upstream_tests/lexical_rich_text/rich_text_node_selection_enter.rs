//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionEnter.test.ts`.

#![cfg(feature = "extension")]

use super::support::{build_with_decorators, select_nodes};
use crate::extension::LexicalEditorWithDispose;
use crate::upstream_tests::support::create_test_decorator_node_inline;
use crate::{KEY_ENTER_COMMAND, NodeRef, get_root, get_selection};

/// `$isParagraphNode(node)`.
fn is_paragraph_node(node: &NodeRef) -> bool {
    node.get_type() == "paragraph"
}

fn create_editor(inline: bool) -> LexicalEditorWithDispose {
    build_with_decorators(move || {
        let decorator = create_test_decorator_node_inline(inline);
        get_root().append(std::slice::from_ref(&decorator))?;
        select_nodes(&[decorator]);
        Ok(())
    })
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionEnter.test.ts::KEY_ENTER_COMMAND on a single-decorator NodeSelection > block decorator: inserts a paragraph after it and moves the caret
#[test]
fn block_decorator_inserts_a_paragraph_after_it_and_moves_the_caret() {
    let editor = create_editor(false);
    editor.dispatch_command(&KEY_ENTER_COMMAND, None).unwrap();
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 2);
            let children = root.get_children();
            assert!(children[0].is_decorator());
            assert!(is_paragraph_node(&children[1]));
            let selection = get_selection().and_then(|s| s.range()).expect("a range");
            assert_eq!(selection.anchor.key, children[1].get_key());
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionEnter.test.ts::KEY_ENTER_COMMAND on a single-decorator NodeSelection > inline decorator: no-op
#[test]
fn inline_decorator_no_op() {
    let editor = create_editor(true);
    editor.dispatch_command(&KEY_ENTER_COMMAND, None).unwrap();
    editor
        .read(|| {
            // inline nodes are wrapped
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert!(children.iter().all(is_paragraph_node));
            let paragraph_children = children[0].get_children();
            assert_eq!(paragraph_children.len(), 1);
            assert!(paragraph_children[0].is_decorator());
            assert!(get_selection().is_some_and(|s| s.is_node()));
        })
        .unwrap();
    editor.dispose();
}
