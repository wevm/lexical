//! `@lexical/utils` (the `utils` feature): what of it the ported packages
//! use so far.
//!
//! From `packages/lexical-utils/src/index.ts`.

use crate::caret::CaretDirection;
use crate::lexical_selection::get_selection;
use crate::selection::is_at_edge_of_element;
use crate::{LexicalError, LexicalNode, NodeRef, Point};
use std::collections::HashSet;

/// `$findMatchingParent(startingNode, findFn)`: `starting_node` or its
/// nearest ancestor below the root for which `find_fn` is true.
pub use crate::selection_insert_nodes::find_matching_parent;

/// `$getNearestNodeOfType(node, klass)`: `node` or its nearest ancestor that
/// is a `T`.
pub fn get_nearest_node_of_type<T: LexicalNode>(node: &NodeRef) -> Option<NodeRef> {
    let mut parent = Some(node.clone());
    while let Some(p) = parent {
        if p.get_latest().is::<T>() {
            return Some(p);
        }
        parent = p.get_parent();
    }
    None
}

/// `$isAtStartOfNode(point, node)`: whether the collapsed `point` sits at
/// the very start of `node`'s content.
pub fn is_at_start_of_node(point: &Point, node: &NodeRef) -> Result<bool, LexicalError> {
    is_at_edge_of_element(point, node, CaretDirection::Previous)
}

/// `$isAtEndOfNode(point, node)`: whether the collapsed `point` sits at the
/// very end of `node`'s content.
pub fn is_at_end_of_node(point: &Point, node: &NodeRef) -> Result<bool, LexicalError> {
    is_at_edge_of_element(point, node, CaretDirection::Next)
}

/// `$handleIndentAndOutdent(indentOrOutdent)`: applies `indent_or_outdent`
/// once to each block (non-inline element) holding a node of the range
/// selection whose `canIndent()` is true. Returns whether it handled one.
pub fn handle_indent_and_outdent(
    indent_or_outdent: impl Fn(&NodeRef) -> Result<(), LexicalError>,
) -> Result<bool, LexicalError> {
    let Some(selection) = get_selection().filter(|s| s.is_range()) else {
        return Ok(false);
    };
    let mut already_handled: HashSet<String> = HashSet::new();
    for node in selection.get_nodes()? {
        if already_handled.contains(node.get_key()) {
            continue;
        }
        let Some(parent_block) =
            find_matching_parent(&node, |parent| parent.is_element() && !parent.is_inline())
        else {
            continue;
        };
        let parent_key = parent_block.get_key().to_owned();
        if parent_block.can_indent() && !already_handled.contains(&parent_key) {
            already_handled.insert(parent_key);
            indent_or_outdent(&parent_block)?;
        }
    }
    Ok(!already_handled.is_empty())
}
