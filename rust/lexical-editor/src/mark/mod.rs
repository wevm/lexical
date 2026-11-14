//! `@lexical/mark` (the `mark` feature), from `packages/lexical-mark/src`:
//! `MarkNode` with its generated JSON (`MarkNode.ts`,
//! `LexicalMarkGeneratedJSON.ts`), `$wrapSelectionInMarkNode`,
//! `$unwrapMarkNode` and `$getMarkIDs` (`index.ts`), and the functions of
//! `MarkStateExtension.ts` that need no extension, which keep mark ids as
//! the `markIds` NodeState of the leaves they cover instead of wrapping
//! them (`$addMarkID`, `$removeMarkID`, `$getNodeMarkIDs`,
//! `$getMarkIDsAtSelection`, `$migrateMarkNodesToMarkState`), with the
//! default `markableNodes`.
//!
//! Not ported: `createDOM` and `updateDOM`, which come with `html`;
//! `MarkExtension` (its `nodes` are [`mark_nodes`]); and the parts of
//! `MarkStateExtension.ts` that need the extension API or a DOM (rust/PARITY.md).

mod generated_json;
mod mark_node;
mod mark_state_extension;

pub use mark_node::{MarkNode, create_mark_node, is_mark_node};
pub use mark_state_extension::{
    MARK_STATE_KEY, MarkIDsMode, add_mark_id, get_mark_ids_at_selection, get_node_mark_ids,
    is_inline_decorator_node, migrate_mark_nodes_to_mark_state, remove_mark_id,
};

use crate::lexical_selection::{SelectionRef, Which, create_range_selection};
use crate::node::NodeRef;
use crate::{LexicalError, NodeClass};

/// The node classes of this package (`MarkExtension`'s `nodes`):
/// `MarkNode`.
pub fn mark_nodes() -> Vec<NodeClass> {
    vec![NodeClass::of::<MarkNode>()]
}

/// `$unwrapMarkNode(node)`: puts the mark's children in its place.
pub fn unwrap_mark_node(node: &NodeRef) -> Result<(), LexicalError> {
    let mut target: Option<NodeRef> = None;
    for child in node.get_children() {
        match &target {
            None => node.insert_before(&child, true)?,
            Some(target) => target.insert_after(&child, true)?,
        };
        target = Some(child);
    }
    node.remove(false)
}

/// `createNode` of `$wrapSelectionInMarkNode`: a mark with the given ids.
type CreateNode = dyn Fn(&[String]) -> NodeRef;

/// `$wrapSelectionInMarkNode(selection, isBackward, id, createNode)`: wraps
/// the text and the inline elements and decorators `selection` covers in
/// marks with `id` (made by `create_node`, or [`create_mark_node`]), one per
/// run of siblings. A line break or a block ends a run; a mark in
/// the range is not wrapped itself, its text is (in a mark of its own,
/// inside it). The selection is collapsed at the end of the last mark (its
/// start when `is_backward`).
pub fn wrap_selection_in_mark_node(
    selection: &SelectionRef,
    is_backward: bool,
    id: &str,
    create_node: Option<&CreateNode>,
) -> Result<(), LexicalError> {
    // Force a forwards selection since append is used, ignore the argument.
    // A new selection is used to avoid side-effects of flipping the given
    // selection
    let forward_selection = create_range_selection();
    let (start, end) = if selection.is_backward()? {
        (Which::Focus, Which::Anchor)
    } else {
        (Which::Anchor, Which::Focus)
    };
    for (which, from) in [(Which::Anchor, start), (Which::Focus, end)] {
        let point = selection.point(from).expect("a range selection");
        forward_selection.point_set(which, &point.key, point.offset, point.point_type)?;
    }

    let mut current_node_parent: Option<NodeRef> = None;
    let mut last_created_mark_node: Option<NodeRef> = None;

    // Note that extract will split text nodes at the boundaries
    for node in forward_selection.extract()? {
        if last_created_mark_node
            .as_ref()
            .is_some_and(|mark| mark.is_parent_of(&node))
        {
            // If the current node is a child of the last created mark node,
            // there is nothing to do here
            continue;
        }
        let target_node = if node.is_text() {
            // Case 1: The node is a text node and we can include it
            Some(node)
        } else if is_mark_node(&node) {
            // Case 2: the node is a mark node and we can ignore it as a
            // target, moving on to its children
            continue;
        } else if (node.is_element() || node.is_decorator()) && node.is_inline() {
            // Case 3: inline element/decorator nodes can be added in their
            // entirety to the new mark
            Some(node)
        } else {
            None
        };

        let Some(target_node) = target_node else {
            // If we don't have a target node to wrap we can clear our state
            // and continue on with the next node
            current_node_parent = None;
            last_created_mark_node = None;
            continue;
        };
        if current_node_parent.as_ref() == Some(&target_node) {
            // The current node is a child of the target node to be wrapped,
            // there is nothing to do here.
            continue;
        }
        let parent_node = target_node.get_parent();
        if parent_node.is_none() || parent_node != current_node_parent {
            // If the parent node is not the current node's parent node, we
            // can clear the last created mark node.
            last_created_mark_node = None;
        }
        current_node_parent = parent_node;

        let mark_node = match &last_created_mark_node {
            Some(mark_node) => mark_node.clone(),
            None => {
                // If we don't have a created mark node, we can make one
                let ids = [id.to_owned()];
                let mark_node = match create_node {
                    Some(create_node) => create_node(&ids),
                    None => create_mark_node(&ids),
                };
                target_node.insert_before(&mark_node, true)?;
                last_created_mark_node = Some(mark_node.clone());
                mark_node
            }
        };
        // Add the target node to be wrapped in the latest created mark node
        mark_node.append(&[target_node])?;
    }
    // Make selection collapsed at the end
    if let Some(mark_node) = last_created_mark_node {
        if is_backward {
            mark_node.select_start()?;
        } else {
            mark_node.select_end()?;
        }
    }
    Ok(())
}

/// `$getMarkIDs(node, offset)`: the ids of the nearest mark `node` is in,
/// or of the mark right after a text node on the way up when `offset` is
/// that text node's size (in UTF-16 code units); `None` when there is none.
pub fn get_mark_ids(node: &NodeRef, offset: usize) -> Option<Vec<String>> {
    let mut current_node = Some(node.clone());
    while let Some(current) = current_node {
        if is_mark_node(&current) {
            return current.get_ids();
        } else if current.is_text()
            && offset == current.get_text_content_size()
            && let Some(next_sibling) = current.get_next_sibling().filter(is_mark_node)
        {
            return next_sibling.get_ids();
        }
        current_node = current.get_parent();
    }
    None
}
