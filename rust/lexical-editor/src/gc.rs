//! `LexicalGC.ts`: dropping the nodes an update detached. Decorators and
//! slots come with their ports.

use crate::editor_state::NodeMap;
use crate::ordered::OrderedMap;
use crate::{Editor, EditorState};

/// `node.isAttached()` on a node map: whether its parents lead to the root.
fn is_attached(node_map: &NodeMap, key: &str) -> bool {
    let mut key = key;
    loop {
        if key == "root" {
            return true;
        }
        match node_map.get(key).and_then(|n| n.base().parent.as_deref()) {
            Some(parent) => key = parent,
            None => return false,
        }
    }
}

/// `$garbageCollectDetachedDeepChildNodes`.
fn garbage_collect_detached_deep_child_nodes(
    parent_key: &str,
    prev_node_map: &NodeMap,
    node_map: &NodeMap,
    node_map_delete: &mut Vec<String>,
    dirty_elements: &mut OrderedMap<bool>,
) {
    let Some(node) = node_map.get(parent_key) else {
        return;
    };
    let Some(element) = node.element() else {
        return;
    };
    let mut child_key = element.first.clone();
    while let Some(key) = child_key {
        let Some(child) = node_map.get(&key) else {
            break;
        };
        let is_element = child.element().is_some();
        // A dirty child element is left to the outer loop over the dirty
        // elements, which visits it (unless deleted before its turn).
        if child.base().parent.as_deref() == Some(parent_key)
            && !(is_element && dirty_elements.contains(&key))
        {
            if is_element {
                garbage_collect_detached_deep_child_nodes(
                    &key,
                    prev_node_map,
                    node_map,
                    node_map_delete,
                    dirty_elements,
                );
            }
            // A node created and dropped in this update leaves the dirty
            // record too.
            if !prev_node_map.contains_key(&key) {
                dirty_elements.delete(&key);
            }
            node_map_delete.push(key.clone());
        }
        child_key = child.base().next.clone();
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
                    &mut dirty.elements,
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
}
