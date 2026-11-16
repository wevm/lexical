//! Ports of
//! `packages/lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts`.
//!
//! Only the `'clone'` mode is ported (rust/PARITY.md); the `mutates=self`
//! cases are skipped.

#![cfg(feature = "link")]

use crate::link::{LinkAttributes, LinkNode, create_link_node};
use crate::selection::slice_selected_text_node_content;
use crate::upstream_tests::support::{create_test_editor, update};
use crate::{
    CreateEditorArgs, Editor, NodeClass, NodeRef, PointType, SelectionRef, Which,
    create_paragraph_node, create_range_selection, create_text_node, get_node_by_key, get_root,
};
use std::rc::Rc;

/// `createInitializedEditor()`.
fn create_initialized_editor() -> Editor {
    let editor = create_test_editor(CreateEditorArgs::default());
    update(&editor, || {
        let bold = create_text_node("56789");
        bold.set_format_type("bold");
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("01234"), bold])?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    editor
}

/// `$createTextSelection(start, end)`.
fn create_text_selection(start: usize, end: usize) -> SelectionRef {
    let mut i = 0;
    let selection = create_range_selection();
    for node in get_root().get_all_text_nodes() {
        let j = i + node.get_text_content_size();
        if start >= i && start <= j {
            selection
                .point_set(Which::Anchor, node.get_key(), start - i, PointType::Text)
                .unwrap();
        }
        if end >= i && end <= j {
            selection
                .point_set(Which::Focus, node.get_key(), end - i, PointType::Text)
                .unwrap();
        }
        i = j;
    }
    selection
}

fn full_size(nodes: &[NodeRef]) -> usize {
    nodes.iter().map(NodeRef::get_text_content_size).sum()
}

fn text_of(node: &Rc<dyn crate::LexicalNode>) -> String {
    node.text().expect("a text node").text.clone()
}

/// `empty equivalent endpoints (offset=$offset, backward=$backward,
/// outer=$outer, mutates=clone)`.
fn empty_equivalent_endpoints(offset: usize, backward: bool, outer: bool) {
    let editor = create_test_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<LinkNode>()],
        ..CreateEditorArgs::default()
    });
    update(&editor, move || {
        let text = create_text_node("jh");
        let link = create_link_node("https://example.com", &LinkAttributes::default());
        link.append(std::slice::from_ref(&text))?;
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&link))?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        let selection = create_range_selection();
        let (text_point, element_point) = if backward {
            (Which::Focus, Which::Anchor)
        } else {
            (Which::Anchor, Which::Focus)
        };
        selection.point_set(text_point, text.get_key(), offset, PointType::Text)?;
        selection.point_set(
            element_point,
            (if outer { &paragraph } else { &link }).get_key(),
            if offset == 0 { 0 } else { 1 },
            PointType::Element,
        )?;
        let latest = text.get_latest();
        let slice = slice_selected_text_node_content(&selection, &text)?;
        assert_eq!(text_of(&slice), "");
        assert!(!Rc::ptr_eq(&slice, &latest));
        assert_eq!(text.get_text_content(), "jh");
        let by_key = get_node_by_key(text.get_key()).unwrap();
        assert!(Rc::ptr_eq(&by_key.get_latest(), &latest));
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=false, outer=false, mutates=clone)
#[test]
fn empty_equivalent_endpoints_offset_0_backward_false_outer_false_mutates_clone() {
    empty_equivalent_endpoints(0, false, false);
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=false, outer=true, mutates=clone)
#[test]
fn empty_equivalent_endpoints_offset_0_backward_false_outer_true_mutates_clone() {
    empty_equivalent_endpoints(0, false, true);
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=true, outer=false, mutates=clone)
#[test]
fn empty_equivalent_endpoints_offset_0_backward_true_outer_false_mutates_clone() {
    empty_equivalent_endpoints(0, true, false);
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=true, outer=true, mutates=clone)
#[test]
fn empty_equivalent_endpoints_offset_0_backward_true_outer_true_mutates_clone() {
    empty_equivalent_endpoints(0, true, true);
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=false, outer=false, mutates=clone)
#[test]
fn empty_equivalent_endpoints_offset_2_backward_false_outer_false_mutates_clone() {
    empty_equivalent_endpoints(2, false, false);
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=false, outer=true, mutates=clone)
#[test]
fn empty_equivalent_endpoints_offset_2_backward_false_outer_true_mutates_clone() {
    empty_equivalent_endpoints(2, false, true);
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=true, outer=false, mutates=clone)
#[test]
fn empty_equivalent_endpoints_offset_2_backward_true_outer_false_mutates_clone() {
    empty_equivalent_endpoints(2, true, false);
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=true, outer=true, mutates=clone)
#[test]
fn empty_equivalent_endpoints_offset_2_backward_true_outer_true_mutates_clone() {
    empty_equivalent_endpoints(2, true, true);
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > clone > does not clone with full selection (both nodes)
#[test]
fn does_not_clone_with_full_selection_both_nodes() {
    let editor = create_initialized_editor();
    editor
        .read(|| {
            let text_nodes = get_root().get_all_text_nodes();
            let selection = create_text_selection(0, full_size(&text_nodes));
            for node in &text_nodes {
                let slice = slice_selected_text_node_content(&selection, node).unwrap();
                assert!(Rc::ptr_eq(&slice, &node.get_latest()));
            }
        })
        .unwrap();
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > clone > clones only with partial selection (last node)
#[test]
fn clones_only_with_partial_selection_last_node() {
    let editor = create_initialized_editor();
    editor
        .read(|| {
            let text_nodes = get_root().get_all_text_nodes();
            let selection = create_text_selection(0, full_size(&text_nodes) - 1);
            let last_node = text_nodes.last().unwrap().clone();
            for node in &text_nodes {
                let slice = slice_selected_text_node_content(&selection, node).unwrap();
                if *node == last_node {
                    assert!(!Rc::ptr_eq(&slice, &node.get_latest()));
                    let text = node.get_text_content();
                    assert_eq!(text_of(&slice), text[..text.len() - 1]);
                } else {
                    assert!(Rc::ptr_eq(&slice, &node.get_latest()));
                }
            }
        })
        .unwrap();
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > clone > clones only with partial selection (first node)
#[test]
fn clones_only_with_partial_selection_first_node() {
    let editor = create_initialized_editor();
    editor
        .read(|| {
            let text_nodes = get_root().get_all_text_nodes();
            let selection = create_text_selection(1, full_size(&text_nodes));
            let first_node = text_nodes[0].clone();
            for node in &text_nodes {
                let slice = slice_selected_text_node_content(&selection, node).unwrap();
                if *node == first_node {
                    assert!(!Rc::ptr_eq(&slice, &node.get_latest()));
                    assert_eq!(text_of(&slice), node.get_text_content()[1..]);
                } else {
                    assert!(Rc::ptr_eq(&slice, &node.get_latest()));
                }
            }
        })
        .unwrap();
}

// upstream: lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > clone > can slice a node from both sides
#[test]
fn can_slice_a_node_from_both_sides() {
    let editor = create_initialized_editor();
    editor
        .read(|| {
            let node = get_root().get_all_text_nodes()[0].clone();
            let latest = node.get_latest();
            let original_text = node.get_text_content();
            let selection = create_text_selection(1, 3);
            let mut slice = slice_selected_text_node_content(&selection, &node).unwrap();
            assert!(!Rc::ptr_eq(&slice, &latest));
            assert_eq!(text_of(&slice), node.get_text_content()[1..3]);
            // `slice.setTextContent('different')`: the clone is in no editor
            // state, so it is its own writable version.
            Rc::get_mut(&mut slice)
                .expect("an ephemeral clone")
                .text_mut()
                .unwrap()
                .text = "different".to_owned();
            // no side-effects are observed
            assert_eq!(node.get_text_content(), original_text);
            assert_eq!(text_of(&slice), "different");
            let by_key = get_node_by_key(node.get_key()).unwrap();
            assert!(Rc::ptr_eq(&by_key.get_latest(), &latest));
        })
        .unwrap();
}
