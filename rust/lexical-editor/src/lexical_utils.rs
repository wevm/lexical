//! `LexicalUtils.ts`: keys, node lookup, `getWritable` and the dirty marks
//! it leaves for the update cycle.

use crate::caret_tree::detach_node;
use crate::editor::DirtyType;
use crate::node::{LexicalNode, NodeRef};
use crate::nodes::element::is_root_or_shadow_root;
use crate::nodes::paragraph::create_paragraph_node;
use crate::updates::{
    error_on_infinite_transforms, error_on_read_only, get_active_editor, get_active_editor_state,
};
use crate::{EditorState, LexicalError};
use std::cell::Cell;
use std::rc::Rc;

thread_local! {
    static KEY_COUNTER: Cell<u64> = const { Cell::new(1) };
}

/// `resetRandomKey()`: restarts the key counter, for deterministic keys in
/// tests.
pub fn reset_random_key() {
    KEY_COUNTER.with(|c| c.set(1));
}

/// `generateRandomKey()`.
pub(crate) fn generate_random_key() -> String {
    KEY_COUNTER.with(|c| {
        let key = c.get();
        c.set(key + 1);
        key.to_string()
    })
}

pub(crate) fn get_node_by_key_impl(
    editor_state: &EditorState,
    key: &str,
) -> Option<Rc<dyn LexicalNode>> {
    editor_state.0.node_map.borrow().get(key).cloned()
}

/// `$getNodeByKey(key)`.
pub fn get_node_by_key(key: &str) -> Option<NodeRef> {
    get_node_by_key_impl(&get_active_editor_state(), key).map(|_| NodeRef::new(key))
}

/// `$getNodeByKeyOrThrow(key)`.
pub(crate) fn get_node_by_key_or_throw(key: &str) -> Result<NodeRef, LexicalError> {
    get_node_by_key(key).ok_or_else(|| {
        LexicalError::new(format!(
            "Expected node with key {key} to exist but it's not in the nodeMap."
        ))
    })
}

/// `$getRoot()`.
pub fn get_root() -> NodeRef {
    NodeRef::new("root")
}

/// `$isShadowRootNode(node)`.
pub(crate) fn is_shadow_root_node(node: &NodeRef) -> bool {
    let node = node.get_latest();
    node.element().is_some() && node.is_shadow_root()
}

/// `INTERNAL_$isBlock(node)`: an element that is not a root or shadow
/// root, not inline, can be empty, and has no children or an inline first
/// child. (A block `DecoratorNode` too, once it is ported.)
pub(crate) fn is_block(node: &NodeRef) -> bool {
    if !node.is_element() || is_root_or_shadow_root(&*node.get_latest()) {
        return false;
    }
    let is_leaf_element = node.get_first_child().is_none_or(|first| first.is_inline());
    !node.is_inline() && node.can_be_empty() && is_leaf_element
}

/// `$removeFromParent(node)`: detaches the node without repairing the
/// selection.
pub(crate) fn remove_from_parent(node: &NodeRef) {
    if node.get_parent().is_some() {
        get_writable(node.get_key());
    }
    detach_node(node);
}

/// `$restoreEmptyContainerParagraph(container, removedChild)`: appends an
/// empty paragraph to a root or shadow root a removal left empty, and
/// returns it.
pub(crate) fn restore_empty_container_paragraph(
    container: Option<&NodeRef>,
    removed_child: Option<&NodeRef>,
) -> Result<Option<NodeRef>, LexicalError> {
    let Some(container) = container.filter(|c| is_root_or_shadow_root(&*c.get_latest())) else {
        return Ok(None);
    };
    // Root slots (such as footnotes) do not replace the editable body, once
    // slots are ported.
    if !container.is_attached()
        || !container.is_empty()
        || !(container.is_root() || removed_child.is_some_and(is_block))
    {
        return Ok(None);
    }
    let paragraph = create_paragraph_node();
    container.append(std::slice::from_ref(&paragraph))?;
    Ok(Some(paragraph))
}

/// `$setNodeKey(node, null)`: gives a new node a key, puts it in the active
/// editor state and marks it dirty.
pub(crate) fn set_node_key(mut node: Box<dyn LexicalNode>) -> NodeRef {
    error_on_read_only();
    error_on_infinite_transforms();
    let key = generate_random_key();
    node.base_mut().key = key.clone();
    let is_element = node.element().is_some();
    get_active_editor_state()
        .0
        .node_map
        .borrow_mut()
        .insert(key.clone(), Rc::from(node));
    if let Some(editor) = get_active_editor() {
        editor.mark_dirty(&key, is_element);
    }
    NodeRef::new(key)
}

/// `internalMarkNodeAsDirty(node)`: marks the node's ancestors as dirty
/// (unintentionally, `false`) up to the first one already marked, then the
/// node itself.
pub(crate) fn internal_mark_node_as_dirty(key: &str) {
    error_on_infinite_transforms();
    let Some(editor) = get_active_editor() else {
        return;
    };
    let editor_state = get_active_editor_state();
    let (parent, is_element) = {
        let node_map = editor_state.0.node_map.borrow();
        let node = node_map
            .get(key)
            .unwrap_or_else(|| panic!("internalMarkNodeAsDirty: node {key} does not exist"));
        (node.base().parent.clone(), node.element().is_some())
    };
    {
        let node_map = editor_state.0.node_map.borrow();
        let mut dirty = editor.0.dirty.borrow_mut();
        // `internalMarkParentElementsAsDirty`.
        let mut next_parent = parent;
        while let Some(parent_key) = next_parent {
            if dirty.elements.contains(&parent_key) {
                break;
            }
            let Some(node) = node_map.get(&parent_key) else {
                break;
            };
            dirty.elements.set(&parent_key, false);
            next_parent = node.base().parent.clone();
        }
        if dirty.dirty_type == DirtyType::None {
            dirty.dirty_type = DirtyType::HasDirtyNodes;
        }
    }
    editor.mark_dirty(key, is_element);
}

/// `internalMarkSiblingsAsDirty(node)`.
pub(crate) fn internal_mark_siblings_as_dirty(node: &NodeRef) {
    if let Some(previous) = node.get_previous_sibling() {
        internal_mark_node_as_dirty(previous.get_key());
    }
    if let Some(next) = node.get_next_sibling() {
        internal_mark_node_as_dirty(next.get_key());
    }
}

/// `node.getWritable()`: marks the node dirty, and copies it into the
/// active editor state if anything else (the previous editor state) shares
/// it. Changes then go through [`mutate`].
pub(crate) fn get_writable(key: &str) {
    error_on_read_only();
    if let Some(selection) = get_active_editor_state().0.selection.borrow().as_ref() {
        selection.set_cached_nodes(None);
    }
    unshare(key);
    internal_mark_node_as_dirty(key);
}

fn unshare(key: &str) {
    let editor_state = get_active_editor_state();
    let mut node_map = editor_state.0.node_map.borrow_mut();
    let node = node_map.get_mut(key).unwrap_or_else(|| {
        panic!(
            "Lexical node does not exist in active editor state (key {key}). The reference was probably kept from another read or update."
        )
    });
    if Rc::get_mut(node).is_none() {
        *node = Rc::from(node.clone_node());
    }
}

/// Changes the fields of a node that [`get_writable`] already made writable
/// in this update, as JS assigns to the fields of the writable node, without
/// marking it dirty again.
///
/// `f` runs while the node map is borrowed, so it must only change the
/// node's own fields, not read other nodes.
pub(crate) fn mutate<R>(key: &str, f: impl FnOnce(&mut dyn LexicalNode) -> R) -> R {
    error_on_read_only();
    unshare(key);
    let editor_state = get_active_editor_state();
    let mut node_map = editor_state.0.node_map.borrow_mut();
    let node = node_map.get_mut(key).expect("unshared above");
    f(Rc::get_mut(node).expect("unshared above"))
}

/// `const self = node.getWritable(); f(self)`.
pub(crate) fn with_writable<R>(key: &str, f: impl FnOnce(&mut dyn LexicalNode) -> R) -> R {
    get_writable(key);
    mutate(key, f)
}
