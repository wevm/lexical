//! `caret/LexicalCaretTree.ts`: linking children into their parent.

use crate::LexicalError;
use crate::error::invariant;
use crate::node::{NodeKey, NodeRef};
use crate::utils::with_writable;

/// `$linkSiblings`.
fn link_siblings(parent: &str, previous: Option<NodeKey>, next: Option<NodeKey>) {
    match &previous {
        None => with_writable(parent, |p| element_of(p).first = next.clone()),
        Some(prev) => with_writable(prev, |p| p.base_mut().next = next.clone()),
    }
    match &next {
        None => with_writable(parent, |p| element_of(p).last = previous.clone()),
        Some(n) => with_writable(n, |n| n.base_mut().prev = previous.clone()),
    }
}

fn element_of(node: &mut dyn crate::LexicalNode) -> &mut crate::ElementData {
    node.element_mut().expect("a parent is an element")
}

/// `$insertNodeBetween`.
fn insert_node_between(parent: &str, node: &str, previous: Option<NodeKey>, next: Option<NodeKey>) {
    let key = Some(node.to_owned());
    match &previous {
        None => with_writable(parent, |p| element_of(p).first = key.clone()),
        Some(prev) => with_writable(prev, |p| p.base_mut().next = key.clone()),
    }
    match &next {
        None => with_writable(parent, |p| element_of(p).last = key.clone()),
        Some(n) => with_writable(n, |n| n.base_mut().prev = key.clone()),
    }
    with_writable(node, |n| {
        let base = n.base_mut();
        base.prev = previous;
        base.next = next;
        base.parent = Some(parent.to_owned());
    });
}

/// `$detachNode`.
fn detach_node(node: &NodeRef) {
    let latest = node.get_latest();
    let Some(parent) = latest.base().parent.clone() else {
        return;
    };
    link_siblings(
        &parent,
        latest.base().prev.clone(),
        latest.base().next.clone(),
    );
    with_writable(node.get_key(), |n| {
        let base = n.base_mut();
        base.prev = None;
        base.next = None;
        base.parent = None;
    });
    with_writable(&parent, |p| element_of(p).size -= 1);
}

/// `parent.append(...nodes)`: `ElementNode.splice(size, 0, nodes)`, with
/// `RootNode.splice`'s check. A node that is already a child moves to the
/// end. The deleting half of `splice`, which also repairs the selection,
/// comes with the update API.
pub(crate) fn append(parent: &NodeRef, nodes: &[NodeRef]) -> Result<(), LexicalError> {
    if parent.is_root() {
        for node in nodes {
            // DecoratorNode is allowed too, once it is ported.
            invariant!(
                node.is_element(),
                "rootNode.splice: Only element or decorator nodes can be inserted to the root node"
            );
        }
    }
    for node in nodes {
        invariant!(node != parent, "append: attempting to append self");
        detach_node(node);
        let last = parent.get_last_child().map(|n| n.get_key().to_owned());
        insert_node_between(parent.get_key(), node.get_key(), last, None);
        with_writable(parent.get_key(), |p| element_of(p).size += 1);
    }
    Ok(())
}
