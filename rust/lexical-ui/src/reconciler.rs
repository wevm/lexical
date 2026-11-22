//! `LexicalReconciler.ts`, its model half: the walk `$reconcileRoot` makes
//! over the previous and next trees once the editor has a root element, for
//! what it gives back to the model: the nodes it creates, updates and
//! destroys, for mutation listeners (`setMutatedNode`, in
//! `LexicalUtils.ts`), slot values included (`$mountSlotChildren`,
//! `$reconcileSlotChildren`, `$destroyNode`'s of a host's slots); what each
//! decorator it renders decorates to (`reconcileDecorator`, for
//! `editor._decorators`); and the `textFormat` and `textStyle` a reconciled
//! element takes from its first text descendant (`reconcileTextFormat`,
//! `reconcileTextStyle`). Each node's keyed DOM is a [`ReconciledNode`],
//! which keeps what JS caches on that DOM between commits for the model: an
//! element's first text descendant (`__lexicalFirstTextKey`) and the slot
//! container a slot value is mounted in, and an element's text
//! (`__lexicalTextContent`), so the root's text (`__cachedText`), which a
//! commit compares (a stale one marks the root dirty, in
//! `commit_pending_updates`), is computed again only for the elements the
//! update dirtied ([`root_text_content`]). Nothing is rendered, so the
//! direction caches aren't kept.
//!
//! `$reconcileChildren`'s children fast paths are ported: for an element of
//! four or more children whose size changed by at most one, a dirty suffix
//! of children is reconciled alone (`$tryReconcileSuffixWithSizeDelta` and
//! the same-size suffix), recovering the first text with
//! `$resolveSuffixPathFormat`, and other same-size edits walk the children
//! without reconciling the clean ones. Where JS splices the suffix's text
//! into the parent's cached text, a suffix path records the parent's text
//! before the suffix ([`TextSplice`]), which [`root_text_content`] completes.

use crate::Editor;
use crate::editor::{DirtyType, NodeMutation};
use crate::editor_state::{EditorState, NodeMap};
use crate::json::array_index;
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::nodes::element::is_root_or_shadow_root;
use crate::ordered::{KeyMap, OrderedMap, OrderedSet};
use crate::slot::{SlotMap, slot_host_key_field, slots_field};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

pub(crate) use crate::editor::MutatedNodes;

/// `editor._decorators` (`Record<NodeKey, unknown>`): what each decorator
/// the reconciler rendered returned from
/// [`decorate`](LexicalNode::decorate), by key, in the order JS's object
/// lists its keys (`Object.entries`): array-index keys ascending, which
/// every generated key is, then the others in insertion order. A value is
/// the app's `Rc<dyn Any>`, compared by identity (`Rc::ptr_eq`) as JS
/// compares the object a `decorate` returns (`===`).
#[derive(Clone, Default)]
pub struct Decorators {
    entries: Vec<(NodeKey, Rc<dyn Any>)>,
}

impl Decorators {
    /// `decorators[key]`.
    pub fn get(&self, key: &str) -> Option<&Rc<dyn Any>> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// `Object.entries(decorators)`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Rc<dyn Any>)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// `Object.keys(decorators)`.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `decorators[key] = value`, in JS's property order.
    fn insert(&mut self, key: &str, value: Rc<dyn Any>) {
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value;
            return;
        }
        let at = match array_index(key) {
            Some(index) => self
                .entries
                .iter()
                .position(|(k, _)| array_index(k).is_none_or(|i| i > index))
                .unwrap_or(self.entries.len()),
            None => self.entries.len(),
        };
        self.entries.insert(at, (key.to_owned(), value));
    }

    /// `delete decorators[key]`.
    fn remove(&mut self, key: &str) {
        self.entries.retain(|(k, _)| k != key);
    }
}

impl fmt::Debug for Decorators {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.keys()).finish()
    }
}

/// `$garbageCollectDetachedDecorators(editor, pendingEditorState)`
/// (`LexicalGC.ts`): the decorators of nodes the pending state no longer
/// has are dropped, in `_pendingDecorators` (made from `_decorators` when
/// there is none, as `cloneDecorators` does).
pub(crate) fn garbage_collect_detached_decorators(
    editor: &Editor,
    pending_editor_state: &EditorState,
) {
    let detached: Vec<NodeKey> = {
        let node_map = pending_editor_state.0.node_map.borrow();
        let pending = editor.0.pending_decorators.borrow();
        let current = editor.0.decorators.borrow();
        pending
            .as_ref()
            .unwrap_or(&current)
            .keys()
            .filter(|key| !node_map.contains_key(key))
            .map(str::to_owned)
            .collect()
    };
    if detached.is_empty() {
        return;
    }
    let pending = editor.0.pending_decorators.borrow_mut().take();
    let mut decorators = pending.unwrap_or_else(|| editor.0.decorators.borrow().clone());
    for key in &detached {
        decorators.remove(key);
    }
    *editor.0.pending_decorators.borrow_mut() = Some(decorators);
}

/// The stand-in for a node's keyed DOM (a value of `editor._keyToDOMMap`):
/// what the reconciler keeps on it between commits.
#[derive(Default)]
pub(crate) struct ReconciledNode {
    /// `__lexicalFirstTextKey` (elements only): `Some(None)` when there is
    /// no text descendant, `None` while unset.
    first_text_key: RefCell<Option<Option<NodeKey>>>,
    /// The slot container the DOM is mounted in (a slot value's
    /// `parentElement`), by its id; `None` in a parent's children.
    container: Cell<Option<u64>>,
    /// `__lexicalTextContent` (elements only): the element's text as of the
    /// last commit that computed it, `None` once an update dirties it.
    text_content: RefCell<Option<String>>,
}

#[cfg(all(test, feature = "extension", feature = "rich-text", feature = "link"))]
impl ReconciledNode {
    /// `dom.__lexicalFirstTextKey`, for the ported reconciler tests.
    pub(crate) fn first_text_key(&self) -> Option<Option<NodeKey>> {
        self.first_text_key.borrow().clone()
    }
}

/// `editor._keyToDOMMap`.
pub(crate) type KeyToDomMap = KeyMap<NodeKey, Rc<ReconciledNode>>;

/// The `DOMSlot` `$createNode` inserts a node into: a slot container, by
/// id, or (`None`) its parent's children.
#[derive(Clone, Copy)]
struct Slot {
    container: Option<u64>,
}

/// `CaptureGuard`: the first text captured before a walk that may reset it.
struct CaptureGuard {
    first_text_key: Option<NodeKey>,
    format: Option<f64>,
    style: Option<String>,
}

/// `MIN_FAST_PATH_CHILDREN`: the fewest children the children fast paths
/// of `$reconcileChildren` engage for.
const MIN_FAST_PATH_CHILDREN: usize = 4;

/// A suffix fast path's half of JS's text splice: the element's cached text
/// without its old dirty suffix, and the first key of the new suffix, whose
/// text [`root_text_content`] appends.
pub(crate) struct TextSplice {
    prefix: String,
    suffix_start: NodeKey,
}

/// The splices of a commit, by element key.
pub(crate) type TextSplices = KeyMap<NodeKey, TextSplice>;

/// What `reconcileTextFormat` and `reconcileTextStyle` set on an element.
struct TextFieldsWrite {
    key: NodeKey,
    format: Option<f64>,
    style: Option<String>,
}

struct Reconciler<'a> {
    editor: &'a Editor,
    /// `treatAllNodesAsDirty`.
    treat_all_nodes_as_dirty: bool,
    /// `activeEditorStateReadOnly`.
    editor_state_read_only: bool,
    dirty_elements: &'a OrderedMap<bool>,
    dirty_leaves: &'a OrderedSet,
    prev_node_map: &'a NodeMap,
    next_node_map: &'a NodeMap,
    /// `activePrevEditorState`, which `$prevSuffixTextSize` reads in.
    prev_editor_state: &'a EditorState,
    /// `activeDirtyChildrenByParent`, made when a fast path first asks.
    dirty_children_by_parent: Option<HashMap<&'a str, HashSet<&'a str>>>,
    /// The text splices the suffix fast paths recorded.
    text_splices: TextSplices,
    /// Whether any mutation listener is registered (`mutationListeners.size`).
    listening: bool,
    mutated_nodes: MutatedNodes,
    /// `activeEditor._keyToDOMMap`, taken from the editor for the walk.
    key_to_dom_map: KeyToDomMap,
    /// `activePrevKeyToDOMMap` (the map as the walk started), as the entries
    /// the walk has changed since: a key's value before its first change.
    prev_key_to_dom_map: HashMap<NodeKey, Option<Rc<ReconciledNode>>>,
    /// `subTreeTextFormat`.
    sub_tree_text_format: Option<f64>,
    /// `subTreeTextStyle`.
    sub_tree_text_style: Option<String>,
    /// `subTreeFirstTextKey`.
    sub_tree_first_text_key: Option<NodeKey>,
    /// The `setTextFormat` and `setTextStyle` calls of `reconcileTextFormat`
    /// and `reconcileTextStyle`, in the walk's order. JS makes them during
    /// the walk; nothing the walk reads later reads them, so they are made
    /// once it ends, when the node maps are no longer borrowed.
    text_fields_writes: Vec<TextFieldsWrite>,
}

impl<'a> Reconciler<'a> {
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

    /// `$beginCaptureGuard()`.
    fn begin_capture_guard(&self) -> CaptureGuard {
        CaptureGuard {
            first_text_key: self.sub_tree_first_text_key.clone(),
            format: self.sub_tree_text_format,
            style: self.sub_tree_text_style.clone(),
        }
    }

    /// `$endCaptureGuard(saved)`: an earlier capture wins.
    fn end_capture_guard(&mut self, saved: CaptureGuard) {
        if saved.first_text_key.is_some() {
            self.sub_tree_text_format = saved.format;
            self.sub_tree_text_style = saved.style;
            self.sub_tree_first_text_key = saved.first_text_key;
        }
    }

    /// The first text of the walk, when none is captured yet: what
    /// `$createChildren` and `$reconcileNodeChildren` do after a child.
    fn capture_first_text(&mut self, key: &str) {
        if self.sub_tree_text_format.is_some() {
            return;
        }
        if let Some(text) = self.next_node_map.get(key).and_then(|n| n.text()) {
            self.sub_tree_text_format = Some(text.get_format());
            self.sub_tree_text_style = Some(text.get_style().to_owned());
            self.sub_tree_first_text_key = Some(key.to_owned());
        }
    }

    /// `$bubbleChildFirstText(childKeyedDom)`: a clean element's cached
    /// first text, unless one is captured already.
    fn bubble_child_first_text(&mut self, child_keyed_dom: &ReconciledNode) {
        if self.sub_tree_first_text_key.is_some() {
            return;
        }
        let child_first_key =
            child_keyed_dom.first_text_key.borrow().clone().expect(
                "$bubbleChildFirstText: missing __lexicalFirstTextKey on element keyed DOM",
            );
        let Some(child_first_key) = child_first_key else {
            return;
        };
        if let Some(text) = self
            .next_node_map
            .get(&child_first_key)
            .and_then(|n| n.text())
        {
            self.sub_tree_text_format = Some(text.get_format());
            self.sub_tree_text_style = Some(text.get_style().to_owned());
            self.sub_tree_first_text_key = Some(child_first_key);
        }
    }

    /// `getElementByKeyOrThrow(activeEditor, key)`.
    fn get_element_by_key_or_throw(&self, key: &str) -> Rc<ReconciledNode> {
        self.key_to_dom_map.get(key).cloned().unwrap_or_else(|| {
            panic!("Reconciliation: could not find DOM element for node key {key}")
        })
    }

    /// `activePrevKeyToDOMMap.get(key)`.
    fn prev_element_by_key(&self, key: &str) -> Option<Rc<ReconciledNode>> {
        match self.prev_key_to_dom_map.get(key) {
            Some(prev) => prev.clone(),
            None => self.key_to_dom_map.get(key).cloned(),
        }
    }

    /// `getPrevElementByKeyOrThrow(key)`.
    fn get_prev_element_by_key_or_throw(&self, key: &str) -> Rc<ReconciledNode> {
        self.prev_element_by_key(key).unwrap_or_else(|| {
            panic!("Reconciliation: could not find DOM element for node key {key}")
        })
    }

    /// Keeps a key's value in the previous map before the walk first changes
    /// it.
    fn save_prev_element(&mut self, key: &str) {
        if !self.prev_key_to_dom_map.contains_key(key) {
            let prev = self.key_to_dom_map.get(key).cloned();
            self.prev_key_to_dom_map.insert(key.to_owned(), prev);
        }
    }

    /// `storeDOMWithKey(key, dom, editor)`.
    fn store_dom_with_key(&mut self, key: &str, dom: Rc<ReconciledNode>) {
        self.save_prev_element(key);
        self.key_to_dom_map.insert(key.to_owned(), dom);
    }

    /// A new slot container (`$createSlotDOM(name)`).
    fn create_slot_dom(&self) -> u64 {
        self.editor.next_id()
    }

    /// `$createChildrenArray(element, nodeMap)`.
    fn children<'m>(node: &dyn LexicalNode, node_map: &'m NodeMap) -> Vec<&'m str> {
        let mut children = Vec::new();
        let mut key = node_map
            .get(node.get_key())
            .and_then(|n| n.element()?.first.as_deref());
        while let Some(k) = key {
            children.push(k);
            key = node_map.get(k).and_then(|n| n.base().next.as_deref());
        }
        children
    }

    /// `$readSlots(node)`.
    fn read_slots(node: &dyn LexicalNode) -> SlotMap {
        slots_field(node).cloned().unwrap_or_default()
    }

    /// `$destroyNode(key, parentDOM)`.
    fn destroy_node(&mut self, key: &str) {
        let node = self.prev_node_map.get(key).cloned();
        // A node moved across parents in the same update still exists in
        // the next map: its new parent reconciles it.
        if self.next_node_map.contains_key(key) {
            return;
        }
        self.save_prev_element(key);
        self.key_to_dom_map.remove(key);
        let Some(node) = node else {
            return;
        };
        if node.element().is_some() {
            let children = Self::children(&*node, self.prev_node_map);
            self.destroy_children(&children);
        }
        // Slots are a separate channel from the children, so destroy each
        // slot subtree too.
        for slot_key in Self::read_slots(&*node).values() {
            self.destroy_node(slot_key);
        }
        self.set_mutated_node(&*node, NodeMutation::Destroyed);
    }

    /// `$destroyChildren(children, startIndex, endIndex, dom)`.
    fn destroy_children(&mut self, children: &[&str]) {
        for child in children {
            self.destroy_node(child);
        }
    }

    /// `$createNode(key, slot)`. With a slot (`slot !== null`), a node the
    /// previous tree had in another parent or slot host, or in a slot
    /// container that is gone, keeps its DOM and is reconciled instead.
    fn create_node(&mut self, key: &str, slot: Option<Slot>) -> Rc<ReconciledNode> {
        let node = self
            .next_node_map
            .get(key)
            .cloned()
            .expect("createNode: node does not exist in nodeMap");
        if let Some(slot) = slot
            && let Some(prev_node) = self.prev_node_map.get(key)
            && let Some(existing_dom) = self.prev_element_by_key(key)
        {
            let prev_slot_host = slot_host_key_field(&**prev_node);
            let next_slot_host = slot_host_key_field(&*node);
            let model_moved =
                prev_node.base().parent != node.base().parent || prev_slot_host != next_slot_host;
            let slot_child_dom_detached =
                next_slot_host.is_some() && existing_dom.container.get() != slot.container;
            if model_moved || slot_child_dom_detached {
                // `slot.insertChild(existingDOM)`.
                existing_dom.container.set(slot.container);
                return self.reconcile_node(key);
            }
        }
        let dom = Rc::new(ReconciledNode::default());
        dom.container.set(slot.and_then(|s| s.container));
        self.store_dom_with_key(key, dom.clone());
        if let Some(element) = node.element() {
            // Slots mount first, ahead of the children.
            let slots = Self::read_slots(&*node);
            if slots.len() > 0 {
                self.mount_slot_children(&slots);
            }
            if element.size == 0 {
                *dom.first_text_key.borrow_mut() = Some(None);
            } else {
                let children = Self::children(&*node, self.next_node_map);
                self.create_children(&children, key);
            }
        } else if node.is_decorator() {
            if let Some(decorator) = node.decorate(self.editor) {
                self.reconcile_decorator(key, decorator);
            }
            let slots = Self::read_slots(&*node);
            if slots.len() > 0 {
                self.mount_slot_children(&slots);
            }
        }
        self.set_mutated_node(&*node, NodeMutation::Created);
        dom
    }

    /// `$createChildren(children, element, startIndex, endIndex, slot)`,
    /// for the keys of `element_key`'s children in `children`.
    fn create_children(&mut self, children: &[&str], element_key: &str) {
        let outer_saved = self.begin_capture_guard();
        self.sub_tree_text_format = None;
        self.sub_tree_text_style = None;
        self.sub_tree_first_text_key = None;
        for child in children {
            let saved = self.begin_capture_guard();
            self.create_node(child, Some(Slot { container: None }));
            self.capture_first_text(child);
            self.end_capture_guard(saved);
        }
        let cache_dom = self.key_to_dom_map.get(element_key).unwrap_or_else(|| {
            panic!("$createChildren: Element with key {element_key} missing from keyToDOMMap")
        });
        *cache_dom.first_text_key.borrow_mut() = Some(self.sub_tree_first_text_key.clone());
        self.end_capture_guard(outer_saved);
    }

    /// `$mountSlotChildren(node, hostDom, slots)`: each slot value is
    /// created in a new slot container.
    fn mount_slot_children(&mut self, slots: &SlotMap) {
        let outer_saved = self.begin_capture_guard();
        for slot_key in slots.values() {
            let container = self.create_slot_dom();
            let saved = self.begin_capture_guard();
            self.create_node(
                slot_key,
                Some(Slot {
                    container: Some(container),
                }),
            );
            self.end_capture_guard(saved);
        }
        self.end_capture_guard(outer_saved);
    }

    /// `$reconcileSlotChildren(prevNode, nextNode, hostDom)`: a removed slot's
    /// value is destroyed; a kept one is reconciled in its container, or
    /// replaced there; a new one is created in a new container.
    fn reconcile_slot_children(
        &mut self,
        prev_node: &dyn LexicalNode,
        next_node: &dyn LexicalNode,
    ) {
        let prev_slots = Self::read_slots(prev_node);
        let next_slots = Self::read_slots(next_node);
        for (name, prev_slot_key) in prev_slots.iter() {
            if next_slots.get(name).is_none() {
                self.destroy_node(prev_slot_key);
            }
        }
        let outer_saved = self.begin_capture_guard();
        for (name, next_slot_key) in next_slots.iter() {
            let prev_slot_key = prev_slots.get(name);
            // `$slotContainerForKey(prevSlotKey)`.
            let container = prev_slot_key
                .and_then(|k| self.prev_element_by_key(k))
                .and_then(|dom| dom.container.get());
            let saved = self.begin_capture_guard();
            match (container, prev_slot_key) {
                (Some(_), Some(prev_slot_key)) if prev_slot_key == next_slot_key => {
                    self.reconcile_node(next_slot_key);
                }
                (Some(container), prev_slot_key) => {
                    if let Some(prev_slot_key) = prev_slot_key {
                        self.destroy_node(prev_slot_key);
                    }
                    self.create_node(
                        next_slot_key,
                        Some(Slot {
                            container: Some(container),
                        }),
                    );
                }
                (None, _) => {
                    let container = self.create_slot_dom();
                    self.create_node(
                        next_slot_key,
                        Some(Slot {
                            container: Some(container),
                        }),
                    );
                }
            }
            self.end_capture_guard(saved);
        }
        self.end_capture_guard(outer_saved);
    }

    /// `reconcileDecorator(key, decorator)`: a decorator that changed is
    /// written to `_pendingDecorators`, made from `_decorators` at the first.
    fn reconcile_decorator(&mut self, key: &str, decorator: Rc<dyn Any>) {
        let editor = self.editor;
        let pending = editor.0.pending_decorators.borrow_mut().take();
        let mut pending = match pending {
            Some(pending) => pending,
            None => {
                let unchanged = editor
                    .0
                    .decorators
                    .borrow()
                    .get(key)
                    .is_some_and(|current| Rc::ptr_eq(current, &decorator));
                if unchanged {
                    return;
                }
                // `cloneDecorators(activeEditor)`.
                editor.0.decorators.borrow().clone()
            }
        };
        pending.insert(key, decorator);
        *editor.0.pending_decorators.borrow_mut() = Some(pending);
    }

    /// `reconcileTextFormat(element)` and `reconcileTextStyle(element)`.
    fn reconcile_text_fields(&mut self, element: &dyn LexicalNode) {
        if self.editor_state_read_only {
            return;
        }
        let data = element.element().expect("an element");
        let format = self
            .sub_tree_text_format
            .filter(|&format| format != data.get_text_format());
        let style = self
            .sub_tree_text_style
            .clone()
            .filter(|style| style != data.get_text_style());
        if format.is_some() || style.is_some() {
            self.text_fields_writes.push(TextFieldsWrite {
                key: element.get_key().to_owned(),
                format,
                style,
            });
        }
    }

    /// `$reconcileChildrenWithDirection(prevElement, nextElement, dom,
    /// derivesTextFields)`.
    fn reconcile_children_with_direction(
        &mut self,
        prev_element: &dyn LexicalNode,
        next_element: &dyn LexicalNode,
        derives_text_fields: bool,
    ) {
        self.sub_tree_text_format = None;
        self.sub_tree_text_style = None;
        self.sub_tree_first_text_key = None;
        self.reconcile_children(prev_element, next_element);
        // The root and shadow roots never expose their text fields.
        if !is_root_or_shadow_root(next_element) && derives_text_fields {
            self.reconcile_text_fields(next_element);
        }
    }

    /// `$reconcileChildren(prevElement, nextElement, slot)`.
    fn reconcile_children(&mut self, prev: &dyn LexicalNode, next: &dyn LexicalNode) {
        let next_key = next.get_key();
        let cache_dom = self
            .key_to_dom_map
            .get(next_key)
            .cloned()
            .unwrap_or_else(|| {
                panic!("$reconcileChildren: Element with key {next_key} missing from keyToDOMMap")
            });
        if self.reconcile_children_fast_path(prev, next, &cache_dom) {
            return;
        }
        let prev_children = Self::children(prev, self.prev_node_map);
        let next_children = Self::children(next, self.next_node_map);
        if prev_children.len() == 1 && next_children.len() == 1 {
            let (prev_first, next_first) = (&prev_children[0], &next_children[0]);
            if prev_first == next_first {
                self.reconcile_node(prev_first);
            } else {
                self.get_prev_element_by_key_or_throw(prev_first);
                self.create_node(next_first, None);
                self.destroy_node(prev_first);
            }
            self.capture_first_text(next_first);
        } else if prev_children.is_empty() {
            if !next_children.is_empty() {
                self.create_children(&next_children, next_key);
            }
        } else if next_children.is_empty() {
            self.destroy_children(&prev_children);
        } else {
            self.reconcile_node_children(next_key, &prev_children, &next_children);
        }
        *cache_dom.first_text_key.borrow_mut() = Some(self.sub_tree_first_text_key.clone());
    }

    /// The children fast paths of `$reconcileChildren`: true when one
    /// reconciled the children, false to take the general walk.
    fn reconcile_children_fast_path(
        &mut self,
        prev: &dyn LexicalNode,
        next: &dyn LexicalNode,
        cache_dom: &ReconciledNode,
    ) -> bool {
        let (Some(prev_element), Some(next_element)) = (prev.element(), next.element()) else {
            return false;
        };
        let prev_size = prev_element.size;
        let size_delta = next_element.size as isize - prev_size as isize;
        // A full reconcile swaps node maps without dirtying what changed. A
        // same-size parent cloned this update may have a stale `__last`
        // (`_cloneNotNeeded`); a ±1 parent is always cloned.
        let cloned = !self
            .prev_node_map
            .get(prev.get_key())
            .zip(self.next_node_map.get(next.get_key()))
            .is_some_and(|(p, n)| Rc::ptr_eq(p, n));
        if self.treat_all_nodes_as_dirty
            || size_delta.abs() > 1
            || prev_size < MIN_FAST_PATH_CHILDREN
            || prev_element.first != next_element.first
            || (size_delta == 0 && cloned)
        {
            return false;
        }
        // The cached text is the element's own concatenation of its
        // children: it is never filled for an element whose text is custom
        // (`$hasCustomTextContent`).
        let has_cached_text = cache_dom.text_content.borrow().is_some();
        let next_key = next.get_key();
        let dirty_children = self.dirty_children(next_key);
        if has_cached_text
            && let Some(dirty_children) = &dirty_children
            && let Some(suffix_start) = self.suffix_start_if_contiguous(next, dirty_children)
        {
            let k = dirty_children.len();
            let prev_suffix = if size_delta == 0 {
                // Same keys in the same order: the prev suffix is this one.
                let suffix = self.suffix_keys(self.next_node_map, suffix_start, k);
                for &key in &suffix {
                    let saved = self.begin_capture_guard();
                    self.reconcile_node(key);
                    self.capture_first_text(key);
                    self.end_capture_guard(saved);
                }
                Some(suffix)
            } else {
                self.try_reconcile_suffix_with_size_delta(prev, suffix_start, k, size_delta)
            };
            if let Some(prev_suffix) = prev_suffix {
                self.record_text_splice(next, cache_dom, &prev_suffix, suffix_start);
                self.resolve_suffix_path_format(next_key, cache_dom, dirty_children);
                return true;
            }
        }
        if size_delta != 0 {
            return false;
        }
        // The structurally clean walk: only dirty children are reconciled;
        // a clean element gives its cached first text.
        let mut key = next_element.first.as_deref();
        while let Some(child) = key {
            let Some(node) = self.next_node_map.get(child) else {
                break;
            };
            let is_dirty = self.dirty_leaves.contains(child) || self.dirty_elements.contains(child);
            let saved = self.begin_capture_guard();
            if is_dirty {
                self.reconcile_node(child);
            } else if node.element().is_some() {
                let dom = self.prev_element_by_key(child).unwrap_or_else(|| {
                    panic!("reconcileChildren structurally-clean walk: missing DOM for {child}")
                });
                self.bubble_child_first_text(&dom);
            }
            self.capture_first_text(child);
            self.end_capture_guard(saved);
            key = node.base().next.as_deref();
        }
        *cache_dom.first_text_key.borrow_mut() = Some(self.sub_tree_first_text_key.clone());
        true
    }

    /// `activeDirtyChildrenByParent.get(key)` (`$buildDirtyChildrenByParent`):
    /// the dirty elements and leaves whose parent is `key`.
    fn dirty_children(&mut self, key: &str) -> Option<HashSet<&'a str>> {
        let next_node_map = self.next_node_map;
        let (elements, leaves) = (self.dirty_elements, self.dirty_leaves);
        let map = self.dirty_children_by_parent.get_or_insert_with(|| {
            let mut map: HashMap<&'a str, HashSet<&'a str>> = HashMap::new();
            for key in elements.iter().map(|(k, _)| k).chain(leaves.iter()) {
                if let Some(node) = next_node_map.get(key)
                    && let Some(parent) = node.base().parent.as_deref()
                {
                    map.entry(parent).or_default().insert(key);
                }
            }
            map
        });
        map.get(key).cloned()
    }

    /// The `count` keys from `start` on, by `__next` in `node_map`.
    fn suffix_keys(&self, node_map: &'a NodeMap, start: &'a str, count: usize) -> Vec<&'a str> {
        let mut keys = Vec::with_capacity(count);
        let mut key = Some(start);
        while let Some(k) = key.filter(|_| keys.len() < count) {
            keys.push(k);
            key = node_map.get(k).and_then(|n| n.base().next.as_deref());
        }
        keys
    }

    /// `$suffixStartIfContiguous(parent, dirty)`: the first of the dirty
    /// children when they are the last of `parent`'s, and not all of them.
    fn suffix_start_if_contiguous(
        &self,
        parent: &dyn LexicalNode,
        dirty: &HashSet<&'a str>,
    ) -> Option<&'a str> {
        let k = dirty.len();
        let element = self.next_node_map.get(parent.get_key())?.element()?;
        if k == 0 || k >= element.size {
            return None;
        }
        let mut cur = element.last.as_deref();
        let mut suffix_start = None;
        let mut i = 0;
        while let Some(key) = cur.filter(|_| i < k) {
            if !dirty.contains(key) {
                return None;
            }
            suffix_start = Some(key);
            cur = self.next_node_map.get(key)?.base().prev.as_deref();
            i += 1;
        }
        if i != k || cur.is_some_and(|key| dirty.contains(key)) {
            return None;
        }
        suffix_start
    }

    /// `$tryReconcileSuffixWithSizeDelta(...)`: a ±1 change of the children
    /// confined to the dirty suffix (an append or a split at the end, K = 2;
    /// a merge at the end, K = 1) reconciles only the suffix. Gives the
    /// previous suffix's keys, or `None` to take the general walk.
    fn try_reconcile_suffix_with_size_delta(
        &mut self,
        prev: &dyn LexicalNode,
        suffix_start: &'a str,
        k: usize,
        size_delta: isize,
    ) -> Option<Vec<&'a str>> {
        if k != if size_delta == 1 { 2 } else { 1 } {
            return None;
        }
        let k_prime = (k as isize - size_delta) as usize;
        let prev_node_map = self.prev_node_map;
        let mut prev_suffix_start = prev_node_map
            .get(prev.get_key())?
            .element()?
            .last
            .as_deref();
        for _ in 0..k_prime - 1 {
            prev_suffix_start = prev_node_map
                .get(prev_suffix_start?)?
                .base()
                .prev
                .as_deref();
        }
        let prev_suffix_start = prev_suffix_start?;
        let next_start = self.next_node_map.get(suffix_start)?;
        let prev_start = prev_node_map.get(prev_suffix_start)?;
        if next_start.base().prev != prev_start.base().prev {
            return None;
        }
        let next_suffix = self.suffix_keys(self.next_node_map, suffix_start, k);
        let prev_suffix = self.suffix_keys(prev_node_map, prev_suffix_start, k_prime);
        if next_suffix.len() != k || prev_suffix.len() != k_prime {
            return None;
        }
        enum Op<'k> {
            Reconcile(&'k str),
            Create(&'k str),
            Destroy(&'k str),
        }
        let mut ops = Vec::new();
        let (mut pi, mut ni) = (0, 0);
        while pi < k_prime && ni < k {
            if next_suffix[ni] == prev_suffix[pi] {
                ops.push(Op::Reconcile(next_suffix[ni]));
                pi += 1;
                ni += 1;
            } else if !next_suffix.contains(&prev_suffix[pi]) {
                ops.push(Op::Destroy(prev_suffix[pi]));
                pi += 1;
            } else if !prev_suffix.contains(&next_suffix[ni]) {
                ops.push(Op::Create(next_suffix[ni]));
                ni += 1;
            } else {
                return None;
            }
        }
        ops.extend(prev_suffix[pi..].iter().map(|&key| Op::Destroy(key)));
        ops.extend(next_suffix[ni..].iter().map(|&key| Op::Create(key)));
        for op in ops {
            let saved = self.begin_capture_guard();
            match op {
                Op::Reconcile(key) => {
                    self.reconcile_node(key);
                    self.capture_first_text(key);
                }
                Op::Destroy(key) => self.destroy_node(key),
                Op::Create(key) => {
                    self.create_node(key, Some(Slot { container: None }));
                    self.capture_first_text(key);
                }
            }
            self.end_capture_guard(saved);
        }
        Some(prev_suffix)
    }

    /// `$resolveSuffixPathFormat(nextElement, dom, dirtyChildren)`: the
    /// first text's format and style come from the clean prefix when it has
    /// the element's cached first text; otherwise the suffix's stand, and
    /// become the cache.
    fn resolve_suffix_path_format(
        &mut self,
        parent_key: &str,
        dom: &ReconciledNode,
        dirty_children: &HashSet<&str>,
    ) {
        let cached_first_text_key = dom.first_text_key.borrow().clone().flatten();
        if let Some(cached_first_text_key) = cached_first_text_key {
            let mut ancestor = Some(cached_first_text_key.as_str());
            while let Some(key) = ancestor {
                let Some(node) = self.next_node_map.get(key) else {
                    ancestor = None;
                    break;
                };
                if node.base().parent.as_deref() == Some(parent_key) {
                    break;
                }
                ancestor = node.base().parent.as_deref();
            }
            if ancestor.is_some_and(|key| !dirty_children.contains(key))
                && let Some(text) = self
                    .next_node_map
                    .get(&cached_first_text_key)
                    .and_then(|n| n.text())
            {
                self.sub_tree_text_format = Some(text.get_format());
                self.sub_tree_text_style = Some(text.get_style().to_owned());
                return;
            }
        }
        *dom.first_text_key.borrow_mut() = Some(self.sub_tree_first_text_key.clone());
    }

    /// The text half of a suffix path (`$prevSuffixTextSize` and the
    /// splice): the element's cached text without its previous suffix's,
    /// read in the previous state. Nothing is recorded, so the text is made
    /// again in full, for an element with slots, a previous suffix child
    /// whose text isn't known from its cache or itself, or a cached text
    /// that doesn't end in the previous suffix's.
    fn record_text_splice(
        &mut self,
        next: &dyn LexicalNode,
        cache_dom: &ReconciledNode,
        prev_suffix: &[&str],
        suffix_start: &str,
    ) {
        if Self::read_slots(next).len() > 0 {
            return;
        }
        let Some(cached) = cache_dom.text_content.borrow().clone() else {
            return;
        };
        let prev_node_map = self.prev_node_map;
        let suffix_text =
            crate::updates::read_editor_state(Some(self.editor), self.prev_editor_state, || {
                let mut text = String::new();
                for (i, &key) in prev_suffix.iter().enumerate() {
                    let node = prev_node_map.get(key)?;
                    if node.element().is_some() {
                        text.push_str(
                            self.prev_element_by_key(key)?
                                .text_content
                                .borrow()
                                .as_ref()?,
                        );
                        if i + 1 < prev_suffix.len() && !node.is_inline() {
                            text.push_str(crate::constants::DOUBLE_LINE_BREAK);
                        }
                    } else if Self::read_slots(&**node).len() > 0 {
                        return None;
                    } else {
                        text.push_str(&node.get_text_content());
                    }
                }
                Some(text)
            });
        let Some(prefix) =
            suffix_text.and_then(|suffix| cached.strip_suffix(&suffix[..]).map(str::to_owned))
        else {
            return;
        };
        self.text_splices.insert(
            next.get_key().to_owned(),
            TextSplice {
                prefix,
                suffix_start: suffix_start.to_owned(),
            },
        );
    }

    /// `$reconcileNodeChildren(nextElement, prevChildren, nextChildren,
    /// prevChildrenLength, nextChildrenLength, slot)`.
    fn reconcile_node_children(
        &mut self,
        next_element_key: &str,
        prev_children: &[&str],
        next_children: &[&str],
    ) {
        let mut prev_children_set: Option<HashSet<&str>> = None;
        let mut next_children_set: Option<HashSet<&str>> = None;
        let (mut prev_index, mut next_index) = (0, 0);
        while prev_index < prev_children.len() && next_index < next_children.len() {
            let prev_key = &prev_children[prev_index];
            let next_key = &next_children[next_index];
            let saved = self.begin_capture_guard();
            if prev_key == next_key {
                self.reconcile_node(next_key);
                prev_index += 1;
                next_index += 1;
            } else {
                let next_set = next_children_set
                    .get_or_insert_with(|| next_children[next_index..].iter().copied().collect());
                let prev_set = match &mut prev_children_set {
                    None => prev_children_set
                        .insert(prev_children[prev_index..].iter().copied().collect()),
                    Some(set) => {
                        if !set.contains(*prev_key) {
                            // Already moved.
                            prev_index += 1;
                            self.end_capture_guard(saved);
                            continue;
                        }
                        set
                    }
                };
                if !next_set.contains(*prev_key) {
                    // Remove prev.
                    prev_set.remove(*prev_key);
                    prev_index += 1;
                    self.get_prev_element_by_key_or_throw(prev_key);
                    self.destroy_node(prev_key);
                    self.end_capture_guard(saved);
                    continue;
                }
                if !prev_set.contains(*next_key) {
                    // Create next.
                    next_index += 1;
                    self.create_node(next_key, Some(Slot { container: None }));
                } else {
                    // Move next.
                    prev_index += 1;
                    next_index += 1;
                    self.get_element_by_key_or_throw(next_key);
                    self.reconcile_node(next_key);
                }
            }
            self.capture_first_text(next_key);
            self.end_capture_guard(saved);
        }
        let append_new_children = prev_index >= prev_children.len();
        let remove_old_children = next_index >= next_children.len();
        if append_new_children && !remove_old_children {
            self.create_children(&next_children[next_index..], next_element_key);
        } else if remove_old_children && !append_new_children {
            self.destroy_children(&prev_children[prev_index..]);
        }
    }

    /// `$reconcileNode(key, parentDOM)`.
    fn reconcile_node(&mut self, key: &str) -> Rc<ReconciledNode> {
        let (Some(prev_node), Some(next_node)) = (
            self.prev_node_map.get(key).cloned(),
            self.next_node_map.get(key).cloned(),
        ) else {
            panic!("reconcileNode: prevNode or nextNode does not exist in nodeMap");
        };
        let is_dirty = self.treat_all_nodes_as_dirty
            || self.dirty_leaves.contains(key)
            || self.dirty_elements.contains(key);
        let dom = self.get_element_by_key_or_throw(key);
        // The same version in both states, and clean.
        let same = Rc::ptr_eq(&prev_node, &next_node);
        if same && !is_dirty {
            if prev_node.element().is_some() {
                self.bubble_child_first_text(&dom);
            }
            return dom;
        }
        // A cloned node that is dirty was mutated.
        if !same && is_dirty {
            self.set_mutated_node(&*next_node, NodeMutation::Updated);
        }
        if next_node.update_dom(&*prev_node) {
            let replacement_dom = self.create_node(key, None);
            // `parentDOM.replaceChild(replacementDOM, dom)`.
            replacement_dom.container.set(dom.container.get());
            self.destroy_node(key);
            return replacement_dom;
        }
        let has_slots =
            Self::read_slots(&*next_node).len() > 0 || Self::read_slots(&*prev_node).len() > 0;
        if prev_node.element().is_some() {
            // Slot edits dirty the host, so a clean host's slots are
            // unchanged.
            if is_dirty && has_slots {
                self.reconcile_slot_children(&*prev_node, &*next_node);
            }
            if is_dirty {
                // A full reconcile treats every node as dirty, but an
                // element the update did not change, or change inside,
                // keeps its text fields.
                let derives_text_fields = !same || self.dirty_elements.contains(key);
                self.reconcile_children_with_direction(
                    &*prev_node,
                    &*next_node,
                    derives_text_fields,
                );
            } else {
                self.bubble_child_first_text(&dom);
            }
        } else if next_node.is_decorator() {
            if let Some(decorator) = next_node.decorate(self.editor) {
                self.reconcile_decorator(key, decorator);
            }
            if is_dirty && has_slots {
                self.reconcile_slot_children(&*prev_node, &*next_node);
            }
        }
        dom
    }
}

/// `$reconcileRoot(prevEditorState, nextEditorState, editor, dirtyType,
/// dirtyElements, dirtyLeaves)`: the mutated nodes of the commit. Runs in
/// the pending state, where the text fields the walk derives are set.
pub(crate) fn reconcile_root(
    prev_editor_state: &EditorState,
    next_editor_state: &EditorState,
    editor: &Editor,
) -> (MutatedNodes, TextSplices) {
    let key_to_dom_map = std::mem::take(&mut *editor.0.key_to_dom_map.borrow_mut());
    let (mutated_nodes, key_to_dom_map, writes, text_splices) = {
        let dirty = editor.0.dirty.borrow();
        let prev_node_map = prev_editor_state.0.node_map.borrow();
        let next_node_map = next_editor_state.0.node_map.borrow();
        let mut reconciler = Reconciler {
            editor,
            treat_all_nodes_as_dirty: dirty.dirty_type == DirtyType::FullReconcile,
            editor_state_read_only: next_editor_state.0.read_only.get(),
            dirty_elements: &dirty.elements,
            dirty_leaves: &dirty.leaves,
            prev_node_map: &prev_node_map,
            next_node_map: &next_node_map,
            prev_editor_state,
            dirty_children_by_parent: None,
            text_splices: TextSplices::default(),
            listening: editor.has_mutation_listeners(),
            mutated_nodes: Vec::new(),
            key_to_dom_map,
            prev_key_to_dom_map: HashMap::new(),
            sub_tree_text_format: None,
            sub_tree_text_style: None,
            sub_tree_first_text_key: None,
            text_fields_writes: Vec::new(),
        };
        reconciler.reconcile_node("root");
        (
            reconciler.mutated_nodes,
            reconciler.key_to_dom_map,
            reconciler.text_fields_writes,
            reconciler.text_splices,
        )
    };
    *editor.0.key_to_dom_map.borrow_mut() = key_to_dom_map;
    // `element.setTextFormat(subTreeTextFormat)` and
    // `element.setTextStyle(subTreeTextStyle)`, through `getWritable()`.
    for write in writes {
        let element = NodeRef::new(write.key);
        if let Some(format) = write.format {
            element.set_text_format(format);
        }
        if let Some(style) = write.style {
            element.set_text_style(&style);
        }
    }
    (mutated_nodes, text_splices)
}

thread_local! {
    /// The editor whose root text [`root_text_content`] is computing, with
    /// the commit's text splices.
    static TEXT_CONTENT_EDITOR: RefCell<Option<(Editor, TextSplices)>> = const { RefCell::new(None) };
}

/// The root's text in the pending state, after [`reconcile_root`]: what
/// `$reconcileRoot` leaves in `subTreeTextContent`, from the text each
/// element cached on its keyed DOM (`__lexicalTextContent`) when the update
/// didn't dirty it, so a commit doesn't walk the whole document. The
/// elements' own [`get_text_content`](LexicalNode::get_text_content) still
/// make the text, so it is `$getRoot().getTextContent()`.
pub(crate) fn root_text_content(editor: &Editor, text_splices: TextSplices) -> String {
    {
        let dirty = editor.0.dirty.borrow();
        let map = editor.0.key_to_dom_map.borrow();
        if dirty.dirty_type == DirtyType::FullReconcile {
            for dom in map.values() {
                dom.text_content.take();
            }
        } else {
            // A change marks every ancestor of the changed node dirty.
            for (key, _) in dirty.elements.iter() {
                if let Some(dom) = map.get(key) {
                    dom.text_content.take();
                }
            }
        }
    }
    struct Restore(Option<(Editor, TextSplices)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEXT_CONTENT_EDITOR.with(|e| *e.borrow_mut() = self.0.take());
        }
    }
    let _restore =
        Restore(TEXT_CONTENT_EDITOR.with(|e| e.replace(Some((editor.clone(), text_splices)))));
    crate::lexical_utils::get_root().get_text_content()
}

/// `ElementNode.getTextContent()` of the element `key`, made by `compute`,
/// or, while [`root_text_content`] runs, its keyed DOM's cached text. A
/// suffix fast path's splice gives `compute` the text before the suffix and
/// the suffix's first key, to append the suffix's text to.
pub(crate) fn cached_element_text(
    key: &str,
    compute: impl FnOnce(Option<(String, &str)>) -> String,
) -> String {
    let Some(dom) = TEXT_CONTENT_EDITOR.with(|e| {
        let e = e.borrow();
        let (editor, _) = e.as_ref()?;
        editor.0.key_to_dom_map.borrow().get(key).cloned()
    }) else {
        return compute(None);
    };
    if let Some(text) = dom.text_content.borrow().as_ref() {
        return text.clone();
    }
    let splice = TEXT_CONTENT_EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        e.as_mut().and_then(|(_, splices)| splices.remove(key))
    });
    let text = match splice {
        Some(splice) => compute(Some((splice.prefix, &splice.suffix_start))),
        None => compute(None),
    };
    *dom.text_content.borrow_mut() = Some(text.clone());
    text
}

/// `resetEditor`'s `keyNodeMap.clear()` and, with a root element,
/// `setRootElement`'s `this._keyToDOMMap.set('root', nextRootElement)`.
pub(crate) fn reset_key_to_dom_map(editor: &Editor, has_root_element: bool) {
    let mut map = editor.0.key_to_dom_map.borrow_mut();
    map.clear();
    if has_root_element {
        map.insert("root".to_owned(), Rc::new(ReconciledNode::default()));
    }
}
