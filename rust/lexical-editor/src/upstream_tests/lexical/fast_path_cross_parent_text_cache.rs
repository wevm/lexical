//! Ports of `packages/lexical/src/__tests__/unit/FastPathCrossParentTextCache.test.ts`.
//!
//! Rust's reconciler has only the general walk of `$reconcileChildren`
//! (`reconciler.rs`), so these check that it keeps the root's cached text in
//! sync on the same edits.

#![cfg(all(feature = "extension", feature = "rich-text", feature = "link"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::lexical_utils::with_writable;
use crate::link::{LinkAttributes, create_link_node, link_extension};
use crate::nodes::element::ElementData;
use crate::nodes::line_break::create_line_break_node;
use crate::upstream_tests::support::set_test_root_element;
use crate::{
    Editor, EditorReadMode, LexicalExtension, LexicalNode, NodeBase, NodeClass, NodeRef,
    UpdateOptions, create, create_paragraph_node, create_text_node, define_extension, get_root,
};

/// `FlipInlineNode`: an element whose inline-ness is state.
#[derive(Clone, Debug, Default)]
struct FlipInlineNode {
    base: NodeBase,
    element: ElementData,
    inline: bool,
}

impl LexicalNode for FlipInlineNode {
    fn get_type(&self) -> &'static str {
        "flip-inline"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
    fn is_inline(&self) -> bool {
        self.inline
    }
    fn can_be_empty(&self) -> bool {
        false
    }
}

/// `setInline(inline)`.
fn set_inline(node: &NodeRef, inline: bool) {
    with_writable(node.get_key(), |n| {
        n.downcast_mut::<FlipInlineNode>()
            .expect("a FlipInlineNode")
            .inline = inline;
    });
}

fn build(name: &str, link: bool, nodes: Vec<NodeClass>) -> LexicalEditorWithDispose {
    let mut dependencies = vec![crate::rich_text::rich_text_extension().into()];
    if link {
        dependencies.push(link_extension().into());
    }
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// The root's (cached) text and its children's text joined fresh.
fn cached_and_fresh(editor: &Editor) -> (String, String) {
    editor
        .read_with_mode(EditorReadMode::Latest, || {
            let root = get_root();
            (
                root.get_text_content(),
                root.get_children()
                    .iter()
                    .map(NodeRef::get_text_content)
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
        })
        .unwrap()
}

// upstream: lexical/src/__tests__/unit/FastPathCrossParentTextCache.test.ts::children fast path: cross-parent move and sibling text cache > getTextContent matches the live tree after moving + merging a trailing link
#[test]
fn get_text_content_matches_the_live_tree_after_moving_merging_a_trailing_link() {
    let editor = build("cross-parent-text-cache", true, Vec::new());
    const URL: &str = "https://example.com";
    let mut errors = Vec::new();
    errors.extend(
        editor
            .update(
                || {
                    let root = get_root();
                    root.clear()?;
                    let p0 = create_paragraph_node();
                    let leading_link = create_link_node(URL, &LinkAttributes::default());
                    leading_link.append(&[create_text_node("P0TEXT")])?;
                    p0.append(&[leading_link, create_text_node("after")])?;
                    let p1 = create_paragraph_node();
                    let trailing_link = create_link_node(URL, &LinkAttributes::default());
                    trailing_link.append(&[create_line_break_node()])?;
                    p1.append(&[
                        create_text_node("x"),
                        create_line_break_node(),
                        create_text_node("y"),
                        trailing_link,
                    ])?;
                    root.append(&[p0, p1])
                },
                discrete(),
            )
            .err(),
    );
    errors.extend(
        editor
            .update(
                || {
                    let root = get_root();
                    let p0 = root.get_first_child_or_throw()?;
                    let p1 = root.get_last_child_or_throw()?;
                    assert!(p0.is_element(), "p0 must be an ElementNode");
                    assert!(p1.is_element(), "p1 must be an ElementNode");
                    let trailing_link = p1.get_last_child_or_throw()?;
                    p0.get_first_child_or_throw()?
                        .insert_before(&trailing_link, true)?;
                    Ok(())
                },
                discrete(),
            )
            .err(),
    );
    assert!(errors.is_empty(), "{errors:?}");
    let (cached, fresh) = cached_and_fresh(&editor);
    assert_eq!(cached, fresh);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/FastPathCrossParentTextCache.test.ts::children fast path: cross-parent move and sibling text cache > flipping a non-last suffix element block<->inline keeps the cache in sync
#[test]
fn flipping_a_non_last_suffix_element_block_inline_keeps_the_cache_in_sync() {
    let editor = build(
        "suffix-sibling-isinline",
        false,
        vec![NodeClass::of::<FlipInlineNode>()],
    );
    let mut errors = Vec::new();
    errors.extend(
        editor
            .update(
                || {
                    let root = get_root();
                    root.clear()?;
                    let p = create_paragraph_node();
                    let e = create::<FlipInlineNode>();
                    e.append(&[create_text_node("e")])?;
                    p.append(&[
                        create_text_node("a"),
                        create_line_break_node(),
                        e,
                        create_text_node("z"),
                    ])?;
                    root.append(&[p])
                },
                discrete(),
            )
            .err(),
    );
    errors.extend(
        editor
            .update(
                || {
                    let p = get_root().get_last_child_or_throw()?;
                    assert!(p.is_element(), "p must be an ElementNode");
                    let e = p.get_children()[2].clone();
                    assert!(
                        e.get_latest().is::<FlipInlineNode>(),
                        "e must be a FlipInlineNode"
                    );
                    set_inline(&e, true);
                    let last = p.get_last_child_or_throw()?;
                    assert!(last.is_text(), "last child must be a text node");
                    last.toggle_format("bold");
                    Ok(())
                },
                discrete(),
            )
            .err(),
    );
    assert!(errors.is_empty(), "{errors:?}");
    let (cached, fresh) = cached_and_fresh(&editor);
    assert_eq!(cached, fresh);
    editor.dispose();
}
