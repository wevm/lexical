//! Ports of `packages/lexical/src/__tests__/unit/Issue8922Repro.test.ts`.
//!
//! The model-level cases; the others assert the boundary anchors the DOM
//! reconciler writes (rust/parity/skips.json).

#![cfg(feature = "rich-text")]

use super::super::support::{create_test_editor, set_test_root_element};
use crate::{
    CreateEditorArgs, Editor, LexicalNode, NodeBase, NodeClass, NodeRef, PointType, UpdateOptions,
    Which, create_paragraph_node, create_text_node, get_root, get_selection, select_all,
};
use std::rc::Rc;

/// `TestBlockDecoratorNode` (`test_block_decorator`).
#[derive(Clone, Debug, Default)]
struct TestBlockDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for TestBlockDecoratorNode {
    fn get_type(&self) -> &'static str {
        "test_block_decorator"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        false
    }
    fn decorate(&self, _editor: &Editor) -> Option<Rc<dyn std::any::Any>> {
        None
    }
}

/// `TestInlineDecoratorNode` (`test_inline_decorator`).
#[derive(Clone, Debug, Default)]
struct TestInlineDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for TestInlineDecoratorNode {
    fn get_type(&self) -> &'static str {
        "test_inline_decorator"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn decorate(&self, _editor: &Editor) -> Option<Rc<dyn std::any::Any>> {
        None
    }
}

/// `beforeEach` and `setRootChildren('decorator', 'text', 'decorator')`.
fn set_up() -> Editor {
    let editor = create_test_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<TestBlockDecoratorNode>(),
            NodeClass::of::<TestInlineDecoratorNode>(),
        ],
        ..CreateEditorArgs::default()
    });
    let _ = crate::rich_text::register_rich_text(&editor);
    set_test_root_element(&editor);
    editor
        .update(
            || {
                let decorator = || {
                    crate::lexical_utils::set_node_key(Box::new(TestBlockDecoratorNode::default()))
                };
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("hello")])?;
                let root = get_root();
                root.clear()?;
                root.append(&[decorator(), paragraph, decorator()])
            },
            discrete(),
        )
        .unwrap();
    editor
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// The selection's nodes whose parent is the root are the root's children.
fn expect_spans_every_top_level_node(editor: &Editor) {
    editor
        .read(|| {
            let selection = get_selection().unwrap();
            assert!(selection.is_range());
            let root = get_root();
            let top: Vec<NodeRef> = selection
                .get_nodes()
                .unwrap()
                .into_iter()
                .filter(|n| {
                    n.get_parent()
                        .is_some_and(|p| p.get_key() == root.get_key())
                })
                .collect();
            let keys = |nodes: &[NodeRef]| -> Vec<String> {
                nodes.iter().map(|n| n.get_key().to_owned()).collect()
            };
            assert_eq!(keys(&top), keys(&root.get_children()));
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/Issue8922Repro.test.ts::Issue #8922: select-all with a boundary block decorator > anchors do not shift the child offsets $selectAll resolves to
#[test]
fn anchors_do_not_shift_the_child_offsets_select_all_resolves_to() {
    let editor = set_up();
    editor
        .update(
            || {
                let selection = select_all()?;
                let anchor = selection.point(Which::Anchor).unwrap();
                let focus = selection.point(Which::Focus).unwrap();
                assert_eq!(anchor.key, "root");
                assert_eq!(anchor.point_type, PointType::Element);
                assert_eq!(anchor.offset, 0);
                assert_eq!(focus.key, "root");
                assert_eq!(focus.point_type, PointType::Element);
                assert_eq!(focus.offset, 3);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    expect_spans_every_top_level_node(&editor);
}

// upstream: lexical/src/__tests__/unit/Issue8922Repro.test.ts::Issue #8922: select-all with a boundary block decorator > a DOM selection landing outside the anchors still resolves to the full range
#[test]
fn a_dom_selection_landing_outside_the_anchors_still_resolves_to_the_full_range() {
    let editor = set_up();
    editor
        .update(
            || {
                let selection = select_all()?;
                selection.point_set(Which::Anchor, "root", 0, PointType::Element)?;
                selection.point_set(Which::Focus, "root", 3, PointType::Element)?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    expect_spans_every_top_level_node(&editor);
}
