//! Ports of `packages/lexical-utils/src/__tests__/unit/iterators.test.tsx`.

use super::support::create_headless_test_editor;
use crate::upstream_tests::support::{panic_message, update};
use crate::utils::{first_to_last_iterator, last_to_first_iterator};
use crate::{NodeRef, create_paragraph_node, create_text_node};

/// `$createParagraphNode().splice(0, 0, [0..5 as text nodes])`, checked
/// as upstream does.
fn create_parent() -> NodeRef {
    let parent = create_paragraph_node();
    let children: Vec<NodeRef> = (0..5).map(|i| create_text_node(&i.to_string())).collect();
    parent.splice(0, 0, &children).unwrap();
    // Check initial state
    assert_eq!(texts(&parent), ["0", "1", "2", "3", "4"]);
    parent
}

fn texts(parent: &NodeRef) -> Vec<String> {
    parent
        .get_all_text_nodes()
        .iter()
        .map(NodeRef::get_text_content)
        .collect()
}

fn text_of(node: &NodeRef) -> String {
    assert!(node.is_text(), "Value does not extend TextNode");
    node.get_text_content()
}

// upstream: lexical-utils/src/__tests__/unit/iterators.test.tsx::$firstToLastIterator > Iterates from first to last
#[test]
fn first_to_last_iterates_from_first_to_last() {
    update(&create_headless_test_editor(), || {
        let parent = create_parent();
        let out: Vec<String> = first_to_last_iterator(&parent)
            .map(|n| text_of(&n))
            .collect();
        assert_eq!(out, ["0", "1", "2", "3", "4"]);
        // Parent was not affected
        assert_eq!(texts(&parent), ["0", "1", "2", "3", "4"]);
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/iterators.test.tsx::$firstToLastIterator > Can handle node removal
#[test]
fn first_to_last_can_handle_node_removal() {
    update(&create_headless_test_editor(), || {
        let parent = create_parent();
        let out: Vec<String> = first_to_last_iterator(&parent)
            .map(|node| {
                let rval = text_of(&node);
                node.remove(false).unwrap();
                rval
            })
            .collect();
        assert_eq!(out, ["0", "1", "2", "3", "4"]);
        assert!(parent.get_children().is_empty());
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/iterators.test.tsx::$firstToLastIterator > Detects cycles when nodes move incorrectly
#[test]
fn first_to_last_detects_cycles_when_nodes_move_incorrectly() {
    update(&create_headless_test_editor(), || {
        let parent = create_parent();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            first_to_last_iterator(&parent)
                .map(|node| {
                    let rval = text_of(&node);
                    parent.append(std::slice::from_ref(&node)).unwrap();
                    rval
                })
                .collect::<Vec<_>>()
        }));
        let payload = result.expect_err("a cycle is detected");
        assert!(
            panic_message(payload.as_ref()).contains("$childIterator: Cycle detected"),
            "{}",
            panic_message(payload.as_ref())
        );
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/iterators.test.tsx::$firstToLastIterator > Can handle nodes moving in the other direction
#[test]
fn first_to_last_can_handle_nodes_moving_in_the_other_direction() {
    update(&create_headless_test_editor(), || {
        let parent = create_parent();
        let out: Vec<String> = first_to_last_iterator(&parent)
            .map(|node| {
                let rval = text_of(&node);
                if node.get_previous_sibling().is_some() {
                    parent.splice(0, 0, std::slice::from_ref(&node)).unwrap();
                }
                rval
            })
            .collect();
        assert_eq!(out, ["0", "1", "2", "3", "4"]);
        // This mutation reversed the nodes while traversing
        assert_eq!(texts(&parent), ["4", "3", "2", "1", "0"]);
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/iterators.test.tsx::$lastToFirstIterator > Iterates from last to first
#[test]
fn last_to_first_iterates_from_last_to_first() {
    update(&create_headless_test_editor(), || {
        let parent = create_parent();
        let out: Vec<String> = last_to_first_iterator(&parent)
            .map(|n| text_of(&n))
            .collect();
        assert_eq!(out, ["4", "3", "2", "1", "0"]);
        // Parent was not affected
        assert_eq!(texts(&parent), ["0", "1", "2", "3", "4"]);
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/iterators.test.tsx::$lastToFirstIterator > Can handle node removal
#[test]
fn last_to_first_can_handle_node_removal() {
    update(&create_headless_test_editor(), || {
        let parent = create_parent();
        let out: Vec<String> = last_to_first_iterator(&parent)
            .map(|node| {
                let rval = text_of(&node);
                node.remove(false).unwrap();
                rval
            })
            .collect();
        assert_eq!(out, ["4", "3", "2", "1", "0"]);
        assert!(parent.get_children().is_empty());
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/iterators.test.tsx::$lastToFirstIterator > Can handle nodes moving in the other direction
#[test]
fn last_to_first_can_handle_nodes_moving_in_the_other_direction() {
    update(&create_headless_test_editor(), || {
        let parent = create_parent();
        let out: Vec<String> = last_to_first_iterator(&parent)
            .map(|node| {
                let rval = text_of(&node);
                parent.append(std::slice::from_ref(&node)).unwrap();
                rval
            })
            .collect();
        assert_eq!(out, ["4", "3", "2", "1", "0"]);
        // This mutation reversed the nodes while traversing
        assert_eq!(texts(&parent), ["4", "3", "2", "1", "0"]);
        Ok(())
    });
}
