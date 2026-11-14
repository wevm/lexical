//! `LexicalReconciler.ts`, its model half: the walk `$reconcileRoot` makes
//! over the previous and next trees, for what it reports to mutation
//! listeners (`setMutatedNode`, in `LexicalUtils.ts`): the nodes it creates,
//! updates and destroys. There is no DOM, so nothing is rendered, and the
//! text, format and direction caches the walk fills are not kept.
//!
//! The children fast paths of `$reconcileChildren` (a dirty suffix of four
//! or more children) only save DOM work: they reach the same nodes in the
//! same order as the general walk, which is what this ports. The slot walks
//! (`$mountSlotChildren`, `$reconcileSlotChildren` and `$destroyNode`'s of
//! a host's slot subtrees) aren't ported yet, so a node in a slot reports
//! no mutation.

use crate::Editor;
use crate::editor::{DirtyType, NodeMutation, NodeMutations};
use crate::editor_state::{EditorState, NodeMap};
use crate::node::{LexicalNode, NodeKey};
use crate::ordered::{OrderedMap, OrderedSet};
use std::collections::HashSet;
use std::rc::Rc;

/// `MutatedNodes`: by node type, in the order the walk first reached one.
pub(crate) type MutatedNodes = Vec<(&'static str, NodeMutations)>;

struct Reconciler<'a> {
    /// `treatAllNodesAsDirty`.
    treat_all_nodes_as_dirty: bool,
    dirty_elements: &'a OrderedMap<bool>,
    dirty_leaves: &'a OrderedSet,
    prev_node_map: &'a NodeMap,
    next_node_map: &'a NodeMap,
    /// Whether any mutation listener is registered (`mutationListeners.size`).
    listening: bool,
    mutated_nodes: MutatedNodes,
}

impl Reconciler<'_> {
    /// `setMutatedNode(mutatedNodes, registeredNodes, mutationListeners,
    /// node, mutation)`.
    fn set_mutated_node(&mut self, node: &dyn LexicalNode, mutation: NodeMutation) {
        if !self.listening {
            return;
        }
        let node_type = node.get_type();
        let index = match self.mutated_nodes.iter().position(|(t, _)| *t == node_type) {
            Some(index) => index,
            None => {
                self.mutated_nodes.push((node_type, Vec::new()));
                self.mutated_nodes.len() - 1
            }
        };
        let by_type = &mut self.mutated_nodes[index].1;
        let key = node.get_key();
        match by_type.iter_mut().find(|(k, _)| k == key) {
            // A node destroyed, then made again: a move, so an update.
            Some((_, prev)) => {
                if *prev == NodeMutation::Destroyed && mutation == NodeMutation::Created {
                    *prev = NodeMutation::Updated;
                }
            }
            None => by_type.push((key.to_owned(), mutation)),
        }
    }

    /// `$createChildrenArray(element, nodeMap)`.
    fn children(node: &dyn LexicalNode, node_map: &NodeMap) -> Vec<NodeKey> {
        let mut children = Vec::new();
        let mut key = node.element().and_then(|e| e.first.clone());
        while let Some(k) = key {
            key = node_map.get(&k).and_then(|n| n.base().next.clone());
            children.push(k);
        }
        children
    }

    /// `$destroyNode(key, parentDOM)`.
    fn destroy_node(&mut self, key: &str) {
        // A node moved across parents in the same update still exists in
        // the next map: its new parent reconciles it.
        if self.next_node_map.contains_key(key) {
            return;
        }
        let Some(node) = self.prev_node_map.get(key).cloned() else {
            return;
        };
        if node.element().is_some() {
            let children = Self::children(&*node, self.prev_node_map);
            self.destroy_children(&children);
        }
        self.set_mutated_node(&*node, NodeMutation::Destroyed);
    }

    /// `$destroyChildren(children, startIndex, endIndex, dom)`.
    fn destroy_children(&mut self, children: &[NodeKey]) {
        for child in children {
            self.destroy_node(child);
        }
    }

    /// `$createNode(key, slot)`. `in_slot` is whether the caller inserts the
    /// node into a parent's children (`slot !== null`), where a node moved
    /// from another parent keeps its DOM and is reconciled instead.
    fn create_node(&mut self, key: &str, in_slot: bool) {
        let node = self
            .next_node_map
            .get(key)
            .cloned()
            .expect("createNode: node does not exist in nodeMap");
        if in_slot
            && let Some(prev_node) = self.prev_node_map.get(key)
            && prev_node.base().parent != node.base().parent
        {
            self.reconcile_node(key);
            return;
        }
        if node.element().is_some() {
            let children = Self::children(&*node, self.next_node_map);
            self.create_children(&children);
        }
        self.set_mutated_node(&*node, NodeMutation::Created);
    }

    /// `$createChildren(children, element, startIndex, endIndex, slot)`.
    fn create_children(&mut self, children: &[NodeKey]) {
        for child in children {
            self.create_node(child, true);
        }
    }

    /// `$reconcileChildren(prevElement, nextElement, slot)`.
    fn reconcile_children(&mut self, prev: &dyn LexicalNode, next: &dyn LexicalNode) {
        let prev_children = Self::children(prev, self.prev_node_map);
        let next_children = Self::children(next, self.next_node_map);
        if prev_children.len() == 1 && next_children.len() == 1 {
            let (prev_first, next_first) = (&prev_children[0], &next_children[0]);
            if prev_first == next_first {
                self.reconcile_node(prev_first);
            } else {
                self.create_node(next_first, false);
                self.destroy_node(prev_first);
            }
        } else if prev_children.is_empty() {
            self.create_children(&next_children);
        } else if next_children.is_empty() {
            self.destroy_children(&prev_children);
        } else {
            self.reconcile_node_children(&prev_children, &next_children);
        }
    }

    /// `$reconcileNodeChildren(nextElement, prevChildren, nextChildren,
    /// prevChildrenLength, nextChildrenLength, slot)`.
    fn reconcile_node_children(&mut self, prev_children: &[NodeKey], next_children: &[NodeKey]) {
        let mut prev_children_set: Option<HashSet<&str>> = None;
        let mut next_children_set: Option<HashSet<&str>> = None;
        let (mut prev_index, mut next_index) = (0, 0);
        while prev_index < prev_children.len() && next_index < next_children.len() {
            let prev_key = &prev_children[prev_index];
            let next_key = &next_children[next_index];
            if prev_key == next_key {
                self.reconcile_node(next_key);
                prev_index += 1;
                next_index += 1;
                continue;
            }
            let next_set = next_children_set.get_or_insert_with(|| {
                next_children[next_index..]
                    .iter()
                    .map(String::as_str)
                    .collect()
            });
            let prev_set = match &mut prev_children_set {
                None => prev_children_set.insert(
                    prev_children[prev_index..]
                        .iter()
                        .map(String::as_str)
                        .collect(),
                ),
                Some(set) => {
                    if !set.contains(prev_key.as_str()) {
                        // Already moved.
                        prev_index += 1;
                        continue;
                    }
                    set
                }
            };
            if !next_set.contains(prev_key.as_str()) {
                // Remove prev.
                prev_set.remove(prev_key.as_str());
                prev_index += 1;
                self.destroy_node(prev_key);
                continue;
            }
            if !prev_set.contains(next_key.as_str()) {
                // Create next.
                next_index += 1;
                self.create_node(next_key, true);
            } else {
                // Move next.
                prev_index += 1;
                next_index += 1;
                self.reconcile_node(next_key);
            }
        }
        let append_new_children = prev_index >= prev_children.len();
        let remove_old_children = next_index >= next_children.len();
        if append_new_children && !remove_old_children {
            self.create_children(&next_children[next_index..]);
        } else if remove_old_children && !append_new_children {
            self.destroy_children(&prev_children[prev_index..]);
        }
    }

    /// `$reconcileNode(key, parentDOM)`.
    fn reconcile_node(&mut self, key: &str) {
        let (Some(prev_node), Some(next_node)) = (
            self.prev_node_map.get(key).cloned(),
            self.next_node_map.get(key).cloned(),
        ) else {
            panic!("reconcileNode: prevNode or nextNode does not exist in nodeMap");
        };
        let is_dirty = self.treat_all_nodes_as_dirty
            || self.dirty_leaves.contains(key)
            || self.dirty_elements.contains(key);
        // The same version in both states, and clean.
        let same = Rc::ptr_eq(&prev_node, &next_node);
        if same && !is_dirty {
            return;
        }
        // A cloned node that is dirty was mutated.
        if !same && is_dirty {
            self.set_mutated_node(&*next_node, NodeMutation::Updated);
        }
        if next_node.update_dom(&*prev_node) {
            self.create_node(key, false);
            self.destroy_node(key);
            return;
        }
        if prev_node.element().is_some() && is_dirty {
            self.reconcile_children(&*prev_node, &*next_node);
        }
    }
}

/// `$reconcileRoot(prevEditorState, nextEditorState, editor, dirtyType,
/// dirtyElements, dirtyLeaves)`: the mutated nodes of the commit.
pub(crate) fn reconcile_root(
    prev_editor_state: &EditorState,
    next_editor_state: &EditorState,
    editor: &Editor,
    dirty_type: DirtyType,
    dirty_elements: &OrderedMap<bool>,
    dirty_leaves: &OrderedSet,
) -> MutatedNodes {
    let prev_node_map = prev_editor_state.0.node_map.borrow();
    let next_node_map = next_editor_state.0.node_map.borrow();
    let mut reconciler = Reconciler {
        treat_all_nodes_as_dirty: dirty_type == DirtyType::FullReconcile,
        dirty_elements,
        dirty_leaves,
        prev_node_map: &prev_node_map,
        next_node_map: &next_node_map,
        listening: editor.has_mutation_listeners(),
        mutated_nodes: Vec::new(),
    };
    reconciler.reconcile_node("root");
    reconciler.mutated_nodes
}
