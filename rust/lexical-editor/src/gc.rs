//! `LexicalGC.ts`: dropping the nodes an update detached, with the slot
//! subtrees of a detached host. `$garbageCollectDetachedDecorators` prunes
//! `editor._decorators`, which only the DOM reconciler fills, so it comes
//! with the DOM.

use crate::editor_state::NodeMap;
use crate::ordered::{OrderedMap, OrderedSet};
use crate::slot::{slot_host_key_field, slots_field};
use crate::{Editor, EditorState};

/// `node.isAttached()` on a node map: whether its parents lead to the root.
fn is_attached(node_map: &NodeMap, key: &str) -> bool {
    let mut key = key;
    loop {
        if key == "root" {
            return true;
        }
        match node_map.get(key).and_then(|n| {
            n.base()
                .parent
                .as_deref()
                .or_else(|| slot_host_key_field(&**n).map(String::as_str))
        }) {
            Some(parent) => key = parent,
            None => return false,
        }
    }
}

/// The dirty record `$garbageCollectDetachedDeepChildNodes` prunes: the
/// dirty elements, or (for a decorator host) the dirty leaves.
enum DirtyNodes<'a> {
    Elements(&'a mut OrderedMap<bool>),
    Leaves(&'a mut OrderedSet),
}

impl DirtyNodes<'_> {
    fn has(&self, key: &str) -> bool {
        match self {
            DirtyNodes::Elements(e) => e.contains(key),
            DirtyNodes::Leaves(l) => l.contains(key),
        }
    }

    fn delete(&mut self, key: &str) {
        match self {
            DirtyNodes::Elements(e) => {
                e.delete(key);
            }
            DirtyNodes::Leaves(l) => {
                l.delete(key);
            }
        }
    }
}

/// `$garbageCollectDetachedDeepChildNodes`: the children of a detached
/// node, and its slot subtrees.
fn garbage_collect_detached_deep_child_nodes(
    parent_key: &str,
    prev_node_map: &NodeMap,
    node_map: &NodeMap,
    node_map_delete: &mut Vec<String>,
    dirty_nodes: &mut DirtyNodes,
) {
    let Some(node) = node_map.get(parent_key) else {
        return;
    };
    if let Some(element) = node.element() {
        let mut child_key = element.first.clone();
        while let Some(key) = child_key {
            let Some(child) = node_map.get(&key) else {
                break;
            };
            let is_element = child.element().is_some();
            // A dirty child element is left to the outer loop over the dirty
            // elements, which visits it (unless deleted before its turn).
            if child.base().parent.as_deref() == Some(parent_key)
                && !(is_element && dirty_nodes.has(&key))
            {
                if is_element || slots_field(&**child).is_some() {
                    garbage_collect_detached_deep_child_nodes(
                        &key,
                        prev_node_map,
                        node_map,
                        node_map_delete,
                        dirty_nodes,
                    );
                }
                // A node created and dropped in this update leaves the dirty
                // record too.
                if !prev_node_map.contains_key(&key) {
                    dirty_nodes.delete(&key);
                }
                node_map_delete.push(key.clone());
            }
            child_key = child.base().next.clone();
        }
    }
    // Slot values are not children: reach them through the slot map, when
    // their slot host is this node.
    let slot_keys: Vec<String> = slots_field(&**node)
        .map(|slots| slots.values().cloned().collect())
        .unwrap_or_default();
    for slot_key in slot_keys {
        let Some(slot_node) = node_map.get(&slot_key) else {
            continue;
        };
        if slot_host_key_field(&**slot_node).map(String::as_str) != Some(parent_key) {
            continue;
        }
        if slot_node.element().is_some() || slots_field(&**slot_node).is_some() {
            garbage_collect_detached_deep_child_nodes(
                &slot_key,
                prev_node_map,
                node_map,
                node_map_delete,
                dirty_nodes,
            );
        }
        if !prev_node_map.contains_key(&slot_key) {
            dirty_nodes.delete(&slot_key);
        }
        node_map_delete.push(slot_key);
    }
}

/// `$garbageCollectDetachedNodes(prevEditorState, editorState, dirtyLeaves,
/// dirtyElements)`: removes the detached dirty nodes, and their subtrees,
/// from `editor_state`.
pub(crate) fn garbage_collect_detached_nodes(
    prev_editor_state: &EditorState,
    editor_state: &EditorState,
    editor: &Editor,
) {
    let prev_node_map = prev_editor_state.0.node_map.borrow();
    let mut node_map_delete = Vec::new();
    {
        let node_map = editor_state.0.node_map.borrow();
        let mut dirty = editor.0.dirty.borrow_mut();
        let mut i = 0;
        while let Some(key) = dirty.elements.entry_at(&mut i).map(|(k, _)| k.to_owned()) {
            i += 1;
            let Some(node) = node_map.get(&key) else {
                continue;
            };
            if is_attached(&node_map, &key) {
                continue;
            }
            if node.element().is_some() {
                garbage_collect_detached_deep_child_nodes(
                    &key,
                    &prev_node_map,
                    &node_map,
                    &mut node_map_delete,
                    &mut DirtyNodes::Elements(&mut dirty.elements),
                );
            }
            if !prev_node_map.contains_key(&key) {
                dirty.elements.delete(&key);
            }
            node_map_delete.push(key);
        }
        let mut i = 0;
        while let Some(key) = dirty.leaves.key_at(&mut i).map(str::to_owned) {
            i += 1;
            if node_map.contains_key(&key) && !is_attached(&node_map, &key) {
                // A decorator host is a leaf, so the walk above never reaches
                // its slots.
                if node_map
                    .get(&key)
                    .is_some_and(|n| slots_field(&**n).is_some())
                {
                    garbage_collect_detached_deep_child_nodes(
                        &key,
                        &prev_node_map,
                        &node_map,
                        &mut node_map_delete,
                        &mut DirtyNodes::Leaves(&mut dirty.leaves),
                    );
                }
                if !prev_node_map.contains_key(&key) {
                    dirty.leaves.delete(&key);
                }
                node_map_delete.push(key);
            }
        }
    }
    let mut node_map = editor_state.0.node_map.borrow_mut();
    for key in node_map_delete {
        node_map.remove(&key);
    }
    // Clear the composition key if it points at a node that just got
    // collected, so `isComposing()` doesn't keep reporting true.
    let mut composition_key = editor.0.composition_key.borrow_mut();
    if composition_key
        .as_ref()
        .is_some_and(|key| !node_map.contains_key(key))
    {
        *composition_key = None;
    }
}
