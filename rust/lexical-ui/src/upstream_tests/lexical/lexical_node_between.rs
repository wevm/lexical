//! Ports of `packages/lexical/src/__tests__/unit/LexicalNodeBetween.test.ts`.

#![cfg(all(feature = "extension", feature = "link"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{LinkAttributes, create_link_node, link_extension};
use crate::upstream_tests::support::{update, update_with};
use crate::{
    AnyLexicalExtensionArgument, LexicalExtension, LexicalNode, NodeRef, create_paragraph_node,
    create_text_node, define_extension, get_root,
};
use std::rc::Rc;

fn build(name: &str, dependencies: Vec<AnyLexicalExtensionArgument>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn keys(nodes: &[NodeRef]) -> Vec<String> {
    nodes.iter().map(|n| n.get_key().to_owned()).collect()
}

/// `node traversal stops at a nested ancestor endpoint (reverse=%s)`.
fn node_traversal_stops_at_a_nested_ancestor_endpoint(reverse: bool) {
    let editor = build("caret-node-between", vec![link_extension().into()]);
    update(&editor, move || {
        let text = create_text_node("inside");
        let link = create_link_node("https://example.test", &LinkAttributes::default());
        link.append(std::slice::from_ref(&text))?;
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&link))?;
        let outside = create_paragraph_node();
        outside.append(&[create_text_node("outside")])?;
        get_root().clear()?;
        get_root().append(&[paragraph.clone(), outside])?;
        let between = if reverse {
            paragraph.get_nodes_between(&text)
        } else {
            text.get_nodes_between(&paragraph)
        };
        assert_eq!(keys(&between), keys(&[text, link, paragraph]));
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalNodeBetween.test.ts::node traversal stops at a nested ancestor endpoint (reverse=false)
#[test]
fn node_traversal_stops_at_a_nested_ancestor_endpoint_reverse_false() {
    node_traversal_stops_at_a_nested_ancestor_endpoint(false);
}

// upstream: lexical/src/__tests__/unit/LexicalNodeBetween.test.ts::node traversal stops at a nested ancestor endpoint (reverse=true)
#[test]
fn node_traversal_stops_at_a_nested_ancestor_endpoint_reverse_true() {
    node_traversal_stops_at_a_nested_ancestor_endpoint(true);
}

/// Upstream's `end` is the object the first update made; the version a
/// [`NodeRef`] resolved then (`end_version`) stands for it.
// upstream: lexical/src/__tests__/unit/LexicalNodeBetween.test.ts::node traversal recognizes a stale target reference
#[test]
fn node_traversal_recognizes_a_stale_target_reference() {
    let editor = build("caret-selection-test", vec![]);
    let (start, end, end_version) = update_with(&editor, || {
        let start = create_text_node("start");
        start.toggle_unmergeable();
        let end = create_text_node("end");
        end.toggle_unmergeable();
        let outside = create_text_node("outside");
        outside.toggle_unmergeable();
        let paragraph = create_paragraph_node();
        paragraph.append(&[start.clone(), end.clone(), outside])?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        let end_version: Rc<dyn LexicalNode> = end.get_latest();
        Ok((start, end, end_version))
    });
    update(&editor, move || {
        end.set_format_type("italic");
        let current_end = end.get_latest();
        assert!(!Rc::ptr_eq(&current_end, &end_version));
        assert_eq!(
            keys(&start.get_nodes_between(&end)),
            keys(&[start.clone(), end.clone()])
        );
        Ok(())
    });
    editor.dispose();
}
