//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/descendantsMatching.test.tsx`.

use super::support::{
    create_headless_test_editor, create_text_and_paragraph_with_depth, text_content_for_depth,
};
use crate::upstream_tests::support::update;
use crate::utils::descendants_matching;
use crate::{NodeRef, create_paragraph_node, get_root};

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
        assert_eq!(first_node.get_type(), "paragraph");
        assert_eq!(
            first_node.get_children().iter().all(NodeRef::is_text),
            depth == 0
        );
        let matching = descendants_matching(&first_node.get_children(), NodeRef::is_text);
        first_node.splice(0, first_node.get_children_size(), &matching)?;
        assert!(first_node.get_children().iter().all(NodeRef::is_text));
        assert_eq!(first_node.get_text_content(), text_content_for_depth(depth));
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/descendantsMatching.test.tsx::$descendantsMatching > Can un-nest children at depth 0
#[test]
fn can_un_nest_children_at_depth_0() {
    can_un_nest_children_at_depth(0);
}

// upstream: lexical-utils/src/__tests__/unit/descendantsMatching.test.tsx::$descendantsMatching > Can un-nest children at depth 1
#[test]
fn can_un_nest_children_at_depth_1() {
    can_un_nest_children_at_depth(1);
}

// upstream: lexical-utils/src/__tests__/unit/descendantsMatching.test.tsx::$descendantsMatching > Can un-nest children at depth 2
#[test]
fn can_un_nest_children_at_depth_2() {
    can_un_nest_children_at_depth(2);
}
