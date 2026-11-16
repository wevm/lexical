//! `LexicalNormalization.ts`: merging adjacent simple text nodes, except
//! into or out of the node being composed in an update tagged
//! `defer-composition-merge`. (`$isEmptyCompositionTarget` is desktop
//! WebKit's, so always false here.)

use crate::LexicalError;
use crate::editor::DEFER_COMPOSITION_MERGE_TAG;
use crate::node::NodeRef;
use crate::node_state::node_states_are_equivalent;
use crate::updates::get_active_editor;
use std::rc::Rc;

/// `$canSimpleTextNodesBeMerged(node1, node2)`: the same mode, format and
/// style, and equivalent NodeState.
fn can_simple_text_nodes_be_merged(node1: &NodeRef, node2: &NodeRef) -> bool {
    let (node1, node2) = (node1.get_latest(), node2.get_latest());
    let (a, b) = (
        node1.text().expect("a text node"),
        node2.text().expect("a text node"),
    );
    let (node1_state, node2_state) = (node1.base().state.as_ref(), node2.base().state.as_ref());
    a.mode == b.mode
        && a.format == b.format
        && a.style == b.style
        && (node1_state
            .zip(node2_state)
            .is_some_and(|(a, b)| Rc::ptr_eq(a, b))
            || node_states_are_equivalent(node1_state, node2_state))
}

/// `$mergeTextNodes(node1, node2)`.
fn merge_text_nodes(node1: &NodeRef, node2: &NodeRef) -> Result<NodeRef, LexicalError> {
    let writable_node1 = node1.merge_with_sibling(node2)?;
    if let Some(editor) = get_active_editor() {
        let mut dirty = editor.0.dirty.borrow_mut();
        dirty.normalized.add(node1.get_key());
        dirty.normalized.add(node2.get_key());
    }
    Ok(writable_node1)
}

fn is_mergeable_simple_text(node: &NodeRef) -> bool {
    node.is_text() && node.is_simple_text() && !node.is_unmergeable()
}

/// `$normalizeTextNode(textNode)`: removes the node if it is empty, else
/// merges it with an adjacent compatible text node on each side, removing
/// empty ones on the way.
pub(crate) fn normalize_text_node(text_node: &NodeRef) -> Result<(), LexicalError> {
    let mut node = text_node.clone();
    if node.get_text_content().is_empty() && node.is_simple_text() && !node.is_unmergeable() {
        node.remove(false)?;
        return Ok(());
    }

    // An update tagged DEFER_COMPOSITION_MERGE_TAG never merges text into
    // or out of the node being composed. Once such a merge was held back,
    // later updates of the same composition hold it back too. The nodes
    // merge when the composition ends.
    let editor = get_active_editor();
    let composition_key = editor.as_ref().and_then(|e| e.composition_key());
    let hold_composition = editor.as_ref().is_some_and(|editor| {
        composition_key.is_some()
            && (*editor.0.composition_merge_held.borrow() == composition_key
                || editor.0.tags.borrow().contains(DEFER_COMPOSITION_MERGE_TAG))
    });
    let holds_merge = |node1: &NodeRef, node2: &NodeRef| {
        let key = composition_key.as_deref();
        if let Some(editor) = editor.as_ref().filter(|_| hold_composition)
            && (Some(node1.get_key()) == key || Some(node2.get_key()) == key)
            && can_simple_text_nodes_be_merged(node1, node2)
        {
            *editor.0.composition_merge_held.borrow_mut() = composition_key.clone();
            return true;
        }
        false
    };

    // Backward
    while let Some(previous) = node.get_previous_sibling().filter(is_mergeable_simple_text) {
        if previous.get_text_content().is_empty() {
            previous.remove(false)?;
        } else {
            if !holds_merge(&previous, &node) && can_simple_text_nodes_be_merged(&previous, &node) {
                node = merge_text_nodes(&previous, &node)?;
            }
            break;
        }
    }

    // Forward
    while let Some(next) = node.get_next_sibling().filter(is_mergeable_simple_text) {
        if next.get_text_content().is_empty() {
            next.remove(false)?;
        } else {
            if !holds_merge(&node, &next) && can_simple_text_nodes_be_merged(&node, &next) {
                merge_text_nodes(&node, &next)?;
            }
            break;
        }
    }
    Ok(())
}
