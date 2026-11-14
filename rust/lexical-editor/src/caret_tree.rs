//! `caret/LexicalCaretTree.ts`: linking nodes into and out of their parent,
//! with the selection repairs that go with it.
//!
//! As in JS, the helpers that take "writable" nodes expect the caller to
//! have called `getWritable` on them already ([`get_writable`]), in the
//! order JS does: that order is the order update listeners see the nodes
//! marked dirty in.

use crate::LexicalError;
use crate::error::invariant;
use crate::lexical_selection::{
    PointType, SelectionRef, Which, get_selection, selection_touches_element,
    update_element_selection_on_create_delete_node,
};
use crate::lexical_utils::{get_writable, mutate};
use crate::node::{NodeKey, NodeRef};
use crate::updates::error_on_read_only;

fn set_first(parent: &str, key: Option<NodeKey>) {
    mutate(parent, |p| {
        p.element_mut().expect("a parent is an element").first = key
    });
}

fn set_last(parent: &str, key: Option<NodeKey>) {
    mutate(parent, |p| {
        p.element_mut().expect("a parent is an element").last = key
    });
}

pub(crate) fn add_size(parent: &str, delta: isize) {
    mutate(parent, |p| {
        let element = p.element_mut().expect("a parent is an element");
        element.size = element.size.checked_add_signed(delta).expect("size >= 0");
    });
}

/// `$linkSiblings(writableParent, writablePrevious, writableNext)`.
fn link_siblings(parent: &str, previous: Option<&str>, next: Option<&str>) {
    match previous {
        None => set_first(parent, next.map(str::to_owned)),
        Some(p) => mutate(p, |p| p.base_mut().next = next.map(str::to_owned)),
    }
    match next {
        None => set_last(parent, previous.map(str::to_owned)),
        Some(n) => mutate(n, |n| n.base_mut().prev = previous.map(str::to_owned)),
    }
}

/// `$insertNodeBetween(writableParent, writableNode, writablePrevious,
/// writableNext)`.
pub(crate) fn insert_node_between(
    parent: &str,
    node: &str,
    previous: Option<&str>,
    next: Option<&str>,
) {
    let key = Some(node.to_owned());
    match previous {
        None => set_first(parent, key.clone()),
        Some(p) => mutate(p, |p| p.base_mut().next = key.clone()),
    }
    match next {
        None => set_last(parent, key),
        Some(n) => mutate(n, |n| n.base_mut().prev = Some(node.to_owned())),
    }
    mutate(node, |n| {
        let base = n.base_mut();
        base.prev = previous.map(str::to_owned);
        base.next = next.map(str::to_owned);
        base.parent = Some(parent.to_owned());
    });
}

fn clear_links(node: &str) {
    mutate(node, |n| {
        let base = n.base_mut();
        base.prev = None;
        base.next = None;
        base.parent = None;
    });
}

/// `$detachNode(writableNode)`: unlinks a writable node from its parent.
pub(crate) fn detach_node(node: &NodeRef) {
    let Some(parent) = node.get_parent() else {
        return;
    };
    get_writable(parent.get_key());
    let previous = node.get_previous_sibling();
    let next = node.get_next_sibling();
    if let Some(p) = &previous {
        get_writable(p.get_key());
    }
    if let Some(n) = &next {
        get_writable(n.get_key());
    }
    link_siblings(
        parent.get_key(),
        previous.as_ref().map(NodeRef::get_key),
        next.as_ref().map(NodeRef::get_key),
    );
    clear_links(node.get_key());
    add_size(parent.get_key(), -1);
}

/// `$detachSiblingRange(writableParent, first, count, before)`: unlinks
/// `count` siblings from `first` on, and returns their keys.
pub(crate) fn detach_sibling_range(
    parent: &NodeRef,
    first: Option<NodeRef>,
    count: usize,
    before: Option<&NodeRef>,
) -> Result<Vec<NodeKey>, LexicalError> {
    let mut nodes = Vec::with_capacity(count);
    let mut node = first;
    for _ in 0..count {
        let Some(n) = node else {
            return Err(LexicalError::new("splice: sibling not found"));
        };
        let next = n.get_next_sibling();
        get_writable(n.get_key());
        nodes.push(n);
        node = next;
    }
    if let Some(b) = before {
        get_writable(b.get_key());
    }
    if let Some(n) = &node {
        get_writable(n.get_key());
    }
    link_siblings(
        parent.get_key(),
        before.map(NodeRef::get_key),
        node.as_ref().map(NodeRef::get_key),
    );
    add_size(parent.get_key(), -(nodes.len() as isize));
    Ok(nodes
        .into_iter()
        .map(|n| {
            clear_links(n.get_key());
            n.get_key().to_owned()
        })
        .collect())
}

/// `$detachNodeWithSelection(node, selection)`: detaches a writable node,
/// shifting element points on its parent past it. Returns the points that
/// were just after the node, which `insertAfter` puts after it again.
pub(crate) fn detach_node_with_selection(
    node: &NodeRef,
    selection: Option<&SelectionRef>,
) -> Result<Option<Vec<Which>>, LexicalError> {
    let parent = selection.and_then(|_| node.get_parent());
    // Keep bulk moves linear when no element point observes the index.
    let index = match (selection, &parent) {
        (Some(s), Some(p)) if selection_touches_element(s, p) => node.get_index_within_parent(),
        _ => None,
    };
    let mut points = None;
    if let (Some(selection), Some(parent), Some(index)) = (selection, &parent, index) {
        points = Some(
            [Which::Anchor, Which::Focus]
                .into_iter()
                .filter(|&which| {
                    let point = selection.point(which).expect("a range selection");
                    point.point_type == PointType::Element
                        && point.key == parent.get_key()
                        && point.offset == index + 1
                })
                .collect(),
        );
        // Offset repair needs direction while both points still share a
        // tree.
        selection.is_backward()?;
    }
    detach_node(node);
    if let (Some(selection), Some(parent), Some(index)) = (selection, &parent, index) {
        update_element_selection_on_create_delete_node(selection, parent, index, -1)?;
    }
    if let Some(selection) = selection {
        // Reinsertion can reverse text-only ranges without changing either
        // point.
        selection.with_range(|r| r.cached_is_backward = None);
    }
    Ok(points)
}

/// `errorOnInsertTextNodeOnRoot(node, insertNode)`.
pub(crate) fn error_on_insert_text_node_on_root(
    node: &NodeRef,
    insert_node: &NodeRef,
) -> Result<(), LexicalError> {
    invariant!(
        !node.get_parent().is_some_and(|p| p.is_root()) || insert_node.is_element(),
        "Only element or decorator nodes can be inserted in to the root node"
    );
    Ok(())
}

/// `$insertSibling(origin, direction, node, restoreSelection)`: the shared
/// body of `insertBefore` and `insertAfter`.
pub(crate) fn insert_sibling(
    origin: &NodeRef,
    is_next: bool,
    node: &NodeRef,
    restore_selection: bool,
) -> Result<NodeRef, LexicalError> {
    error_on_read_only();
    error_on_insert_text_node_on_root(origin, node)?;
    get_writable(origin.get_key());
    get_writable(node.get_key());
    origin.get_parent_or_throw()?;
    let selection = get_selection().filter(|s| restore_selection && s.is_range());
    let points = detach_node_with_selection(node, selection.as_ref())?;
    let parent = origin.get_parent_or_throw()?;
    get_writable(parent.get_key());
    // Before insertion this is the new node's index in either direction.
    let index = match &selection {
        Some(s)
            if (is_next && points.as_ref().is_some_and(|p| !p.is_empty()))
                || selection_touches_element(s, &parent) =>
        {
            origin
                .get_index_within_parent()
                .map(|i| i + usize::from(is_next))
        }
        _ => None,
    };
    let sibling = if is_next {
        origin.get_next_sibling()
    } else {
        origin.get_previous_sibling()
    };
    if let Some(s) = &sibling {
        get_writable(s.get_key());
    }
    let sibling_key = sibling.as_ref().map(NodeRef::get_key);
    let (previous, next) = if is_next {
        (Some(origin.get_key()), sibling_key)
    } else {
        (sibling_key, Some(origin.get_key()))
    };
    insert_node_between(parent.get_key(), node.get_key(), previous, next);
    add_size(parent.get_key(), 1);
    if let (Some(selection), Some(index)) = (&selection, index) {
        update_element_selection_on_create_delete_node(selection, &parent, index, 1)?;
        if is_next && let Some(points) = points {
            for which in points {
                selection.point_set(which, parent.get_key(), index + 1, PointType::Element)?;
            }
        }
    }
    Ok(node.clone())
}

/// `$selectAdjacentNode(caret, anchorOffset, focusOffset)`: selects next to
/// `origin`, in the sibling on that side or in the parent.
pub(crate) fn select_adjacent_node(
    origin: &NodeRef,
    is_next: bool,
    anchor_offset: Option<usize>,
    focus_offset: Option<usize>,
) -> Result<SelectionRef, LexicalError> {
    error_on_read_only();
    let sibling = if is_next {
        origin.get_next_sibling()
    } else {
        origin.get_previous_sibling()
    };
    let parent = origin.get_parent_or_throw()?;
    match sibling {
        None if is_next => parent.select(None, None),
        None => parent.select(Some(0), Some(0)),
        Some(sibling) if sibling.is_element() => {
            if is_next {
                sibling.select(Some(0), Some(0))
            } else {
                sibling.select(None, None)
            }
        }
        Some(sibling) if sibling.is_text() => sibling.select(anchor_offset, focus_offset),
        Some(sibling) => {
            let index = sibling.get_index_within_parent().unwrap_or(0) + usize::from(!is_next);
            parent.select(Some(index), Some(index))
        }
    }
}
