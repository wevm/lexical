//! `LexicalSlot.ts` (`@experimental`): named slots, a channel of node
//! subtrees a host (an element or a decorator) holds apart from its
//! children. A slot value's up-link is its host (`__slotHost`), not a parent,
//! so the slot boundary behaves like a shadow root: selection, traversal and
//! editing treat it as one.

use crate::editor::NodeClass;
use crate::error::invariant;
use crate::lexical_selection::SelectionRef;
use crate::lexical_utils::{get_node_by_key, get_writable, mutate};
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::updates::{get_active_editor_state, get_registry_editor};
use crate::{LexicalError, utf16};
use std::cmp::Ordering;
use std::rc::Rc;

/// A host's slots: name to the key of the node in it, in insertion order,
/// as JS's `Map` keeps them. A host's versions share one until a version
/// writes it, as JS's `SLOT_MAP_OWNER` copy-on-write does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SlotMap {
    entries: Vec<(String, NodeKey)>,
}

impl SlotMap {
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn get(&self, name: &str) -> Option<&NodeKey> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, k)| k)
    }

    pub(crate) fn set(&mut self, name: &str, key: NodeKey) {
        match self.entries.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => entry.1 = key,
            None => self.entries.push((name.to_owned(), key)),
        }
    }

    fn delete(&mut self, name: &str) {
        self.entries.retain(|(n, _)| n != name);
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(n, _)| n.as_str())
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &NodeKey> {
        self.entries.iter().map(|(_, k)| k)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &NodeKey)> {
        self.entries.iter().map(|(n, k)| (n.as_str(), k))
    }
}

/// Whether a node version can host slots and be slotted: an element or a
/// decorator.
fn is_slot_node(node: &dyn LexicalNode) -> bool {
    node.element().is_some() || node.is_decorator()
}

/// `$isSlotHost(node)` (`@experimental`): whether the node can host slots,
/// an element or a decorator. What may be slotted is checked by
/// [`set_slot`].
pub fn is_slot_host(node: &NodeRef) -> bool {
    is_slot_node(&*node.get_latest())
}

/// `$isSlotChild(node)` (`@experimental`): whether the node can be slotted,
/// an element or a decorator ([`set_slot`] refuses an inline one).
pub fn is_slot_child(node: &NodeRef) -> bool {
    is_slot_node(&*node.get_latest())
}

/// The slot host key of this version of a node (`__slotHost`), for the
/// walks over a node map (GC, dirty marks).
pub(crate) fn slot_host_key_field(node: &dyn LexicalNode) -> Option<&NodeKey> {
    if is_slot_node(node) {
        node.base().slot_host.as_ref()
    } else {
        None
    }
}

/// The slots of this version of a node, when it has a slot map
/// (`__slots !== null`).
pub(crate) fn slots_field(node: &dyn LexicalNode) -> Option<&SlotMap> {
    if is_slot_node(node) {
        node.base().slots.as_deref()
    } else {
        None
    }
}

/// `$getSlotHostKey(node)`: the key of the host the node is slotted into.
pub(crate) fn get_slot_host_key(node: &NodeRef) -> Option<NodeKey> {
    slot_host_key_field(&*node.get_latest()).cloned()
}

/// `$getSlotHost(node)` (`@experimental`): the host whose named slot holds
/// the node, or `None` when it is not slotted. Kept apart from
/// [`NodeRef::get_parent`], so the slot boundary behaves like a shadow root.
pub fn get_slot_host(node: &NodeRef) -> Option<NodeRef> {
    let slot_host_key = get_slot_host_key(node)?;
    let host = get_node_by_key(&slot_host_key);
    match host {
        Some(host) if is_slot_host(&host) => Some(host),
        _ => panic!("slotHost must be an ElementNode or a DecoratorNode"),
    }
}

/// `$getSlotNameWithinHost(slotChild)` (`@experimental`): the name of the
/// slot the node occupies on its host, or `None` when it is not slotted.
pub fn get_slot_name_within_host(slot_child: &NodeRef) -> Option<String> {
    let host = get_slot_host(slot_child)?;
    get_slot_map(&host)
        .iter()
        .find(|(_, key)| key.as_str() == slot_child.get_key())
        .map(|(name, _)| name.to_owned())
}

/// `$getSlotFrame(node)` (`@experimental`): the slot value (the "slot
/// frame") whose isolated subtree holds the node, the node itself when it
/// is one, or `None` outside every slot.
pub fn get_slot_frame(node: &NodeRef) -> Option<NodeRef> {
    let mut current = Some(node.clone());
    while let Some(node) = current {
        if get_slot_host_key(&node).is_some() {
            return Some(node);
        }
        current = node.get_parent();
    }
    None
}

/// `$getSelectionSlotFrame(selection)` (`@experimental`): the slot frame
/// the selection is in, from a range selection's anchor or the first node
/// of any other selection, or `None` outside every slot (or without a
/// selection).
pub fn get_selection_slot_frame(
    selection: Option<&SelectionRef>,
) -> Result<Option<NodeRef>, LexicalError> {
    let Some(selection) = selection else {
        return Ok(None);
    };
    let anchor_node = match selection.range() {
        Some(range) => Some(range.anchor.get_node()?),
        None => selection.get_nodes()?.into_iter().next(),
    };
    Ok(anchor_node.and_then(|node| get_slot_frame(&node)))
}

/// `$getSlotMap(node)`: the latest slot map, empty for a node without one.
pub(crate) fn get_slot_map(node: &NodeRef) -> SlotMap {
    slots_field(&*node.get_latest())
        .cloned()
        .unwrap_or_default()
}

/// `$getSlotNames(node)` (`@experimental`): the names of the node's
/// occupied slots, in their canonical order.
pub fn get_slot_names(node: &NodeRef) -> Vec<String> {
    get_slot_map(node).names().map(str::to_owned).collect()
}

/// `$getSlot(node, name)` (`@experimental`): the node in the named slot, or
/// `None` when it is empty.
pub fn get_slot(node: &NodeRef, name: &str) -> Option<NodeRef> {
    let key = get_slot_map(node).get(name).cloned()?;
    get_node_by_key(&key)
}

const RESERVED_SLOT_NAMES: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// `getDeclaredSlots(klass)` (`@experimental`): the `slots` a class
/// declares in its `$config()` (its own, or the nearest ancestor's), the
/// canonical order of its slots; empty when it declares none.
pub fn get_declared_slots(klass: &NodeClass) -> &'static [&'static str] {
    (klass.declared_slots)()
}

/// `$getSlotsTextContent(node)`: the text of the node's slots, in slot
/// order, which `getTextContent` reads before the children.
pub(crate) fn get_slots_text_content(node: &dyn LexicalNode) -> String {
    let mut text_content = String::new();
    if let Some(slots) = slots_field(node) {
        for key in slots.values() {
            if let Some(slot) = get_node_by_key(key) {
                text_content.push_str(&slot.get_text_content());
            }
        }
    }
    text_content
}

/// `$getSlotsTextContentSize(node)`: the size of
/// [`get_slots_text_content`], each slot's `getTextContentSize`.
pub(crate) fn get_slots_text_content_size(node: &dyn LexicalNode) -> usize {
    let mut text_content_size = 0;
    if let Some(slots) = slots_field(node) {
        for key in slots.values() {
            if let Some(slot) = get_node_by_key(key) {
                text_content_size += slot.get_text_content_size();
            }
        }
    }
    text_content_size
}

/// `getDeclaredSlotRank(klass)`: declared name to its index, after checking
/// the declaration. `declared` is the host class's (`host.constructor`).
fn get_declared_slot_rank(
    node_type: &str,
    declared: &[&'static str],
) -> Result<Vec<&'static str>, LexicalError> {
    let mut built: Vec<&'static str> = Vec::new();
    for &name in declared {
        invariant!(
            !RESERVED_SLOT_NAMES.contains(&name),
            "getDeclaredSlotRank: {node_type} declares reserved slot name \"{name}\"; __proto__, constructor, and prototype break the plain-object serialization of slots"
        );
        invariant!(
            !built.contains(&name),
            "getDeclaredSlotRank: {node_type} declares slot name \"{name}\" more than once; the canonical order would be ambiguous"
        );
        built.push(name);
    }
    Ok(built)
}

/// `compareSlotNames(a, b, rank)`: declared names first, in declaration
/// order, then the others in UTF-16 code unit order.
fn compare_slot_names(a: &str, b: &str, rank: &[&str]) -> Ordering {
    let rank_a = rank.iter().position(|n| *n == a);
    let rank_b = rank.iter().position(|n| *n == b);
    match (rank_a, rank_b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => utf16::cmp(a, b),
    }
}

/// `$canonicalizeSlotOrder(host)`: puts a writable host's slots back in
/// canonical order after an insertion.
fn canonicalize_slot_order(host: &NodeRef) -> Result<(), LexicalError> {
    let entries = {
        let latest = host.get_latest();
        let Some(slots) = slots_field(&*latest) else {
            return Ok(());
        };
        if slots.len() < 2 {
            return Ok(());
        }
        let rank = get_declared_slot_rank(latest.get_type(), latest.declared_slots())?;
        let names: Vec<&str> = slots.names().collect();
        if names
            .windows(2)
            .all(|w| compare_slot_names(w[0], w[1], &rank) != Ordering::Greater)
        {
            return Ok(());
        }
        let mut entries = slots.entries.clone();
        entries.sort_by(|(a, _), (b, _)| compare_slot_names(a, b, &rank));
        entries
    };
    mutate(host.get_key(), |n| {
        n.base_mut().slots = Some(Rc::new(SlotMap { entries }))
    });
    Ok(())
}

/// `$getWritableSlots(writableHost)`: the slot map of a host already made
/// writable, to change.
fn with_writable_slots(host: &str, f: impl FnOnce(&mut SlotMap)) {
    mutate(host, |n| {
        f(Rc::make_mut(n.base_mut().slots.get_or_insert_default()))
    });
}

/// `$markSlotsUsed()`: latches "this document uses slots" on the editor
/// and on the editor state being built.
pub(crate) fn mark_slots_used() {
    if let Some(editor) = get_registry_editor() {
        editor.0.slots_used.set(true);
    }
    get_active_editor_state().0.slots_used.set(true);
}

/// `$setSlot(host, name, node)` (`@experimental`): puts `node` in the named
/// slot of `host`, replacing (and removing) the node the slot held. The
/// node moves, from a parent or another slot. It must be an element or a
/// decorator that is not inline; the slot link is its shadow boundary.
/// Returns the host.
pub fn set_slot(host: &NodeRef, name: &str, node: &NodeRef) -> Result<NodeRef, LexicalError> {
    invariant!(
        !RESERVED_SLOT_NAMES.contains(&name),
        "$setSlot: \"{name}\" is a reserved slot name; __proto__, constructor, and prototype break the plain-object serialization of slots"
    );
    // JS refuses a host that is not one at compile time.
    invariant!(
        is_slot_host(host),
        "$setSlot: host {} is not an ElementNode or DecoratorNode",
        host.get_key()
    );
    // Re-setting the value a name already holds is a no-op.
    if get_slot_map(host).get(name).map(String::as_str) == Some(node.get_key()) {
        return Ok(host.clone());
    }
    invariant!(
        (node.is_element() || node.is_decorator()) && !node.is_inline(),
        "$setSlot: node {} is not a valid slot value; a slot value must be a non-inline ElementNode or DecoratorNode (the slot link itself is the shadow boundary).",
        node.get_key()
    );
    invariant!(
        !is_slot_ancestor_or_self(node, host),
        "$setSlot: node {} cannot be slotted into {}; a node may not host itself or an ancestor reached through children or slot up-links — the slot up-link would form a cycle that loops isAttached/GC.",
        node.get_key(),
        host.get_key()
    );
    get_writable(host.get_key());
    if let Some(previous_key) = get_slot_map(host).get(name).cloned() {
        detach_slotted_node(&previous_key)?;
    }
    get_writable(node.get_key());
    // Move semantics: a value slotted elsewhere (or under another name of
    // this host) leaves its host's map.
    if let Some(previous_host) = get_slot_host(node) {
        if let Some(previous_name) = get_slot_name_within_host(node) {
            get_writable(previous_host.get_key());
            with_writable_slots(previous_host.get_key(), |slots| {
                slots.delete(&previous_name)
            });
        }
        mutate(node.get_key(), |n| n.base_mut().slot_host = None);
    }
    crate::caret_tree::detach_node(node)?;
    let host_key = host.get_key().to_owned();
    mutate(node.get_key(), |n| n.base_mut().slot_host = Some(host_key));
    let node_key = node.get_key().to_owned();
    with_writable_slots(host.get_key(), |slots| slots.set(name, node_key));
    canonicalize_slot_order(host)?;
    mark_slots_used();
    Ok(host.clone())
}

/// `$removeSlot(host, name)` (`@experimental`): empties the named slot of
/// `host`, removing the node it held. Nothing happens when it is empty.
/// Returns the host.
pub fn remove_slot(host: &NodeRef, name: &str) -> Result<NodeRef, LexicalError> {
    invariant!(
        is_slot_host(host),
        "$removeSlot: host {} is not an ElementNode or DecoratorNode",
        host.get_key()
    );
    get_writable(host.get_key());
    if slots_field(&*host.get_latest()).is_none() {
        return Ok(host.clone());
    }
    if let Some(previous_key) = get_slot_map(host).get(name).cloned() {
        detach_slotted_node(&previous_key)?;
        with_writable_slots(host.get_key(), |slots| slots.delete(name));
    }
    Ok(host.clone())
}

/// `$isSlotAncestorOrSelf(node, host)`: whether `node` is `host` or reached
/// from it through parent and slot up-links.
fn is_slot_ancestor_or_self(node: &NodeRef, host: &NodeRef) -> bool {
    let mut key = Some(host.get_key().to_owned());
    while let Some(k) = key {
        if k == node.get_key() {
            return true;
        }
        let Some(current) = get_node_by_key(&k) else {
            break;
        };
        key = current.get_parent().map(|p| p.get_key().to_owned());
        if key.is_none() {
            key = get_slot_host_key(&current);
        }
    }
    false
}

/// `$errorOnSlotCycleChild(parent, child)`: refuses to make `child` a child
/// of `parent` when `parent` is reached from it through slot up-links (the
/// reverse of [`set_slot`]'s cycle check), once the editor uses slots.
pub(crate) fn error_on_slot_cycle_child(
    parent: &NodeRef,
    child: &NodeRef,
) -> Result<(), LexicalError> {
    if !get_registry_editor().is_some_and(|e| e.0.slots_used.get()) {
        return Ok(());
    }
    invariant!(
        !is_slot_ancestor_or_self(child, parent),
        "insert: node {} cannot become a child of {}; the parent is reachable from the node through slot up-links, so the insertion would form a cycle that loops isAttached/GC.",
        child.get_key(),
        parent.get_key()
    );
    Ok(())
}

/// `$detachSlottedNode(slotKey)`: unslots and removes the node in a slot.
fn detach_slotted_node(slot_key: &str) -> Result<(), LexicalError> {
    let Some(previous) = get_node_by_key(slot_key) else {
        return Ok(());
    };
    get_writable(slot_key);
    invariant!(
        is_slot_child(&previous),
        "detach: slotted node {slot_key} must be an ElementNode or a DecoratorNode"
    );
    mutate(slot_key, |n| n.base_mut().slot_host = None);
    previous.remove(false)
}
