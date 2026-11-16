//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/unwrapAndFilterDescendants.test.tsx`.

use super::support::{
    create_headless_test_editor, create_text_and_paragraph_with_depth, text_content_for_depth,
};
use crate::upstream_tests::support::update;
use crate::utils::unwrap_and_filter_descendants;
use crate::{NodeRef, create_paragraph_node, get_root};

fn is_paragraph_node(node: &NodeRef) -> bool {
    node.get_type() == "paragraph"
}

// upstream: lexical-utils/src/__tests__/unit/unwrapAndFilterDescendants.test.tsx::$unwrapAndFilterDescendants > Is a no-op with valid children
#[test]
fn is_a_no_op_with_valid_children() {
    let editor = create_headless_test_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[create_paragraph_node()])?;
        Ok(())
    });
    update(&editor, || {
        assert!(!unwrap_and_filter_descendants(
            &get_root(),
            is_paragraph_node
        )?);
        assert_eq!(get_root().get_children_size(), 1);
        assert!(
            get_root()
                .get_first_child()
                .is_some_and(|n| is_paragraph_node(&n))
        );
        Ok(())
    });
}

fn can_un_nest_children_at_depth(depth: usize) {
    let editor = create_headless_test_editor();
    update(&editor, move || {
        let first_node = create_paragraph_node();
        first_node.append(&create_text_and_paragraph_with_depth(depth))?;
        let root = get_root();
        root.clear()?;
        root.append(&[first_node])?;
        Ok(())
    });
    update(&editor, move || {
        let first_node = get_root().get_first_child_or_throw()?;
        assert!(is_paragraph_node(&first_node));
        assert_eq!(
            first_node.get_children().iter().all(NodeRef::is_text),
            depth == 0
        );
        assert_eq!(
            unwrap_and_filter_descendants(&first_node, NodeRef::is_text)?,
            depth > 0
        );
        assert!(first_node.get_children().iter().all(NodeRef::is_text));
        assert_eq!(first_node.get_text_content(), text_content_for_depth(depth));
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/unwrapAndFilterDescendants.test.tsx::$unwrapAndFilterDescendants > Can un-nest children at depth 0
#[test]
fn can_un_nest_children_at_depth_0() {
    can_un_nest_children_at_depth(0);
}

// upstream: lexical-utils/src/__tests__/unit/unwrapAndFilterDescendants.test.tsx::$unwrapAndFilterDescendants > Can un-nest children at depth 1
#[test]
fn can_un_nest_children_at_depth_1() {
    can_un_nest_children_at_depth(1);
}

// upstream: lexical-utils/src/__tests__/unit/unwrapAndFilterDescendants.test.tsx::$unwrapAndFilterDescendants > Can un-nest children at depth 2
#[test]
fn can_un_nest_children_at_depth_2() {
    can_un_nest_children_at_depth(2);
}
