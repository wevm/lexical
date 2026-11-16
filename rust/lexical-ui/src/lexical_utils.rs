//! `LexicalUtils.ts`: keys, node lookup, `getWritable` and the dirty marks
//! it leaves for the update cycle.

use crate::caret_tree::detach_node;
use crate::constants::{LTR, RTL};
use crate::editor::DirtyType;
use crate::error::invariant;
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::nodes::element::{Direction, is_root_or_shadow_root as is_root_or_shadow_root_node};
use crate::nodes::paragraph::create_paragraph_node;
use crate::updates::{
    error_on_infinite_transforms, error_on_read_only, get_active_editor, get_active_editor_state,
};
use crate::{Editor, EditorState, LexicalError};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

thread_local! {
    static KEY_COUNTER: Cell<u64> = const { Cell::new(1) };
}

/// `getEditorsToPropagate(editor)`: the editor, then its parent editor,
/// then that one's, up to an editor without one.
pub(crate) fn get_editors_to_propagate(editor: &Editor) -> Vec<Editor> {
    let mut editors_to_propagate = Vec::new();
    let mut current_editor = Some(editor.clone());
    while let Some(editor) = current_editor {
        current_editor = editor.parent_editor().cloned();
        editors_to_propagate.push(editor);
    }
    editors_to_propagate
}

/// `resetRandomKey()`: restarts the key counter, for deterministic keys in
/// tests.
pub fn reset_random_key() {
    KEY_COUNTER.with(|c| c.set(1));
}

/// `createUID()`: five random lowercase letters, as JS takes them from
/// `Math.random().toString(36)` (which can give fewer).
pub(crate) fn create_uid() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut bits = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    (0..5)
        .map(|_| {
            let letter = b'a' + (bits % 26) as u8;
            bits /= 26;
            char::from(letter)
        })
        .collect()
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

/// `INTERNAL_$isBlock(node)`: a `DecoratorNode` that is not inline, or an
/// element that is not a root or shadow root, not inline, can be empty, and
/// has no children or an inline first child.
pub(crate) fn is_block(node: &NodeRef) -> bool {
    if node.is_decorator() && !node.is_inline() {
        return true;
    }
    if !node.is_element() || is_root_or_shadow_root_node(&*node.get_latest()) {
        return false;
    }
    let is_leaf_element = node.get_first_child().is_none_or(|first| first.is_inline());
    !node.is_inline() && node.can_be_empty() && is_leaf_element
}

/// `$removeFromParent(node)`: detaches the node without repairing the
/// selection.
pub(crate) fn remove_from_parent(node: &NodeRef) -> Result<(), LexicalError> {
    if node.get_parent().is_some() {
        get_writable(node.get_key());
    }
    detach_node(node)
}

/// `$restoreEmptyContainerParagraph(container, removedChild)`: appends an
/// empty paragraph to a root or shadow root a removal left empty, and
/// returns it.
pub(crate) fn restore_empty_container_paragraph(
    container: Option<&NodeRef>,
    removed_child: Option<&NodeRef>,
) -> Result<Option<NodeRef>, LexicalError> {
    let Some(container) = container.filter(|c| is_root_or_shadow_root_node(&*c.get_latest()))
    else {
        return Ok(None);
    };
    // Root slots (such as footnotes) do not replace the editable body.
    let has_content = if container.is_root() {
        container.get_children_size() != 0
    } else {
        !container.is_empty()
    };
    if !container.is_attached()
        || has_content
        || !(container.is_root() || removed_child.is_some_and(is_block))
    {
        return Ok(None);
    }
    let paragraph = create_paragraph_node();
    container.append(std::slice::from_ref(&paragraph))?;
    Ok(Some(paragraph))
}

/// `$create(klass)`: a new node of the class `T`, with a key, in the active
/// editor state; or, where the editor replaces the class
/// ([`LexicalNodeReplacement`](crate::LexicalNodeReplacement)), a node of
/// the class its `with_klass` chain ends at, or (for a replacement without
/// `with_klass`) what its `with` makes of the new node. Where no editor
/// registers `T` (outside an editor, or while parsing for one without it)
/// the node is `T`'s, where JS throws.
pub fn create<T: LexicalNode + Default>() -> NodeRef {
    let node_type = T::default().get_type();
    if let Some(editor) = crate::updates::get_registry_editor()
        && let Ok(resolved) = editor.resolve_registered_node_after_replacements(node_type)
        && let Some(class) = editor.get_registered_node(resolved)
        && let Some(node) = class.construct()
    {
        // The resolve follows `withKlass` as far as it goes, so a `replace`
        // still set on the class it stopped at is a deprecated one, whose
        // `with` can only be given a constructed node.
        return if class.replacement.is_some() {
            replaced(node)
        } else {
            node
        };
    }
    set_node_key(Box::new(T::default()))
}

/// `$applyNodeReplacement(node)`: what the active editor's replacement of
/// the node's class (`{replace, with, withKlass}`) makes of the new node,
/// or the node itself when the class has none. Fails as JS's invariants
/// do: for a type the editor doesn't register, or a replacement that isn't
/// of `withKlass` (or, without it, of a class extending the node's), or
/// that kept the node's key.
pub fn apply_node_replacement(node: &NodeRef) -> Result<NodeRef, LexicalError> {
    let editor = crate::editor::get_editor();
    let node_type = node.get_type();
    let node_class_name = node.get_latest().class_name();
    // `errorOnTypeKlassMismatch`, which JS's constructor runs on every node
    // (in development, as the vectors were recorded): its type must be
    // registered, and by its own class. Every node JS hands here was
    // constructed, so it fails this before the invariant below.
    let error_on_type_klass_mismatch = |node_type: &str, class_name: &str| {
        match editor.get_registered_node(node_type) {
            None => invariant!(
                false,
                "Create node: Attempted to create node {class_name} that was not configured to be used on the editor."
            ),
            Some(registered) => invariant!(
                registered.class_name == class_name,
                "Create node: Type {node_type} in node {class_name} does not match registered node {} with the same type",
                registered.class_name
            ),
        }
        Ok(())
    };
    if node_type != "root" {
        error_on_type_klass_mismatch(node_type, node_class_name)?;
    }
    let Some(registered_node) = editor.get_registered_node(node_type) else {
        return Err(LexicalError::new(format!(
            "$applyNodeReplacement node {node_class_name} with type {node_type} must be registered to the editor. You can do this by passing the node class via the \"nodes\" array in the editor config."
        )));
    };
    let Some(replacement) = registered_node.replacement else {
        return Ok(node.clone());
    };
    let replacement_node = (replacement.with)(node)?;
    let replacement_type = replacement_node.get_type();
    let latest = replacement_node.get_latest();
    let class_name = latest.class_name();
    error_on_type_klass_mismatch(replacement_type, class_name)?;
    let is_instance_of =
        |base: &str| replacement_type == base || latest.super_types().contains(&base);
    match &replacement.with_klass {
        Some(with_klass) => invariant!(
            is_instance_of(with_klass.node_type()),
            "$applyNodeReplacement failed. Expected replacement node to be an instance of {} with type {} but returned {} with type {} from original node {} with type {}",
            with_klass.class_name,
            with_klass.node_type(),
            class_name,
            replacement_type,
            node_class_name,
            node_type
        ),
        None => invariant!(
            replacement_type != node_type && is_instance_of(node_type),
            "$applyNodeReplacement failed. Ensure replacement node {} with type {} is a subclass of the original node {} with type {}.",
            class_name,
            replacement_type,
            node_class_name,
            node_type
        ),
    }
    invariant!(
        replacement_node.get_key() != node.get_key(),
        "$applyNodeReplacement failed. Ensure that the key argument is *not* used in your replace function (from node {} with type {} to node {} with type {}), Node keys must never be re-used except by the static clone method.",
        node_class_name,
        node_type,
        class_name,
        replacement_type
    );
    Ok(replacement_node)
}

/// `$applyNodeReplacement(node)` as the `$create*` functions call it: the
/// node itself where no editor is active or the editor doesn't register its
/// class (where JS throws), and a panic where a replacement breaks an
/// invariant (a bug in the app's `with`, as a node implementation's would
/// be).
pub(crate) fn replaced(node: NodeRef) -> NodeRef {
    let Some(editor) = crate::updates::get_registry_editor() else {
        return node;
    };
    if editor
        .get_registered_node(node.get_type())
        .is_none_or(|class| class.replacement.is_none())
    {
        return node;
    }
    apply_node_replacement(&node).unwrap_or_else(|e| panic!("{e}"))
}

/// `$nodesOfType(klass)`, by the node type: the nodes of exactly that type
/// in the active editor state, in its node map's order; in a state still
/// being updated, only those attached to the root.
pub fn nodes_of_type(node_type: &str) -> Vec<NodeRef> {
    let editor_state = get_active_editor_state();
    let read_only = editor_state.0.read_only.get();
    let keys: Vec<String> = {
        let node_map = editor_state.0.node_map.borrow();
        editor_state
            .keys_in_creation_order()
            .into_iter()
            .filter(|key| node_map[key].get_type() == node_type)
            .collect()
    };
    keys.into_iter()
        .map(NodeRef::new)
        .filter(|node| read_only || node.is_attached())
        .collect()
}

/// `$isTokenOrSegmented(node)`: whether a text node is in token or
/// segmented mode.
pub fn is_token_or_segmented(node: &NodeRef) -> bool {
    node.is_token() || node.is_segmented()
}

/// `TypeToNodeMap`: an editor state's nodes by type, then by key.
pub type TypeToNodeMap = HashMap<&'static str, HashMap<NodeKey, Rc<dyn LexicalNode>>>;

/// `getCachedTypeToNodeMap(editorState)`: the nodes of a read-only editor
/// state by type, computed once per state; an empty map for a writable
/// state that is still empty. Panics for any other writable state, as JS's
/// invariant throws.
pub fn get_cached_type_to_node_map(editor_state: &EditorState) -> Rc<TypeToNodeMap> {
    thread_local! {
        static EMPTY_TYPE_TO_NODE_MAP: Rc<TypeToNodeMap> = Rc::default();
    }
    let read_only = editor_state.0.read_only.get();
    if !read_only && editor_state.is_empty() {
        return EMPTY_TYPE_TO_NODE_MAP.with(Rc::clone);
    }
    assert!(
        read_only,
        "getCachedTypeToNodeMap called with a writable EditorState"
    );
    editor_state
        .0
        .type_to_node_map
        .borrow_mut()
        .get_or_insert_with(|| {
            let mut type_to_node_map = TypeToNodeMap::new();
            for (key, node) in editor_state.0.node_map.borrow().iter() {
                type_to_node_map
                    .entry(node.get_type())
                    .or_default()
                    .insert(key.clone(), node.clone());
            }
            Rc::new(type_to_node_map)
        })
        .clone()
}

/// `getRegisteredSubtypeMap(nodes)`: each type of `nodes` to itself and the
/// types of the classes of `nodes` that extend it.
pub fn get_registered_subtype_map(
    nodes: &[crate::NodeClass],
) -> HashMap<&'static str, HashSet<&'static str>> {
    let mut subtypes: HashMap<&'static str, HashSet<&'static str>> = nodes
        .iter()
        .map(|class| (class.node_type, HashSet::new()))
        .collect();
    for class in nodes {
        for own_node_type in std::iter::once(class.node_type).chain((class.super_types)()) {
            if let Some(bucket) = subtypes.get_mut(own_node_type) {
                bucket.insert(class.node_type);
            }
        }
    }
    subtypes
}

/// `$getAdjacentNode(focus, isBackward)`: the node next to the point in the
/// direction, or `None` at a boundary.
pub fn get_adjacent_node(
    focus: &crate::Point,
    is_backward: bool,
) -> Result<Option<NodeRef>, LexicalError> {
    /// `resolveElement(element, isBackward, focusOffset)`.
    fn resolve_element(
        element: &NodeRef,
        is_backward: bool,
        focus_offset: usize,
    ) -> Option<NodeRef> {
        let parent = element.get_parent();
        let mut offset = focus_offset;
        let mut block = element.clone();
        if let Some(parent) = parent {
            if is_backward && focus_offset == 0 {
                offset = block.get_index_within_parent().unwrap_or(0);
                block = parent;
            } else if !is_backward && focus_offset == block.get_children_size() {
                offset = block.get_index_within_parent().unwrap_or(0) + 1;
                block = parent;
            }
        }
        if is_backward {
            offset
                .checked_sub(1)
                .and_then(|i| block.get_child_at_index(i))
        } else {
            block.get_child_at_index(offset)
        }
    }
    let focus_offset = focus.offset;
    let focus_node = focus.get_node()?;
    if focus.point_type == crate::PointType::Element {
        return Ok(resolve_element(&focus_node, is_backward, focus_offset));
    }
    if (is_backward && focus_offset == 0)
        || (!is_backward && focus_offset == focus_node.get_text_content_size())
    {
        let possible_node = if is_backward {
            focus_node.get_previous_sibling()
        } else {
            focus_node.get_next_sibling()
        };
        if possible_node.is_none() {
            let index = focus_node.get_index_within_parent().unwrap_or(0);
            return Ok(resolve_element(
                &focus_node.get_parent_or_throw()?,
                is_backward,
                index + usize::from(!is_backward),
            ));
        }
        return Ok(possible_node);
    }
    Ok(None)
}

/// `$copyNode(node)`: a copy of the node with a new key and all its
/// properties, but no parent, siblings or children, after
/// `resetOnCopyNodeFrom`.
pub fn copy_node(node: &NodeRef) -> NodeRef {
    copy_node_with(node, false)
}

/// `$copyNode(node, skipReset)`: [`copy_node`], without
/// `resetOnCopyNodeFrom` when `skip_reset` is true, so the copy keeps every
/// property and state of the node.
pub fn copy_node_with(node: &NodeRef, skip_reset: bool) -> NodeRef {
    let mut copy = node.get_latest().clone_node();
    let state = copy.base_mut().state.take();
    *copy.base_mut() = crate::node::NodeBase::default();
    if let Some(element) = copy.element_mut() {
        element.first = None;
        element.last = None;
        element.size = 0;
    }
    let copy = set_node_key(copy);
    // `afterCloneFrom` with another key, then `LexicalNode`'s
    // `resetOnCopyNodeFrom`.
    if let Some(state) = state {
        let state = crate::node_state::copy_node_state(&state, copy.get_key(), skip_reset);
        mutate(copy.get_key(), |n| n.base_mut().state = Some(state));
    }
    if !skip_reset {
        copy.get_latest().reset_on_copy_node_from(&copy, node);
    }
    copy
}

/// `$isRootOrShadowRoot(node)`: whether the node is the root or a shadow
/// root.
pub fn is_root_or_shadow_root(node: &NodeRef) -> bool {
    is_root_or_shadow_root_node(&*node.get_latest())
}

/// `$cloneWithProperties(latestNode)`: a clone of the node's latest version
/// with the same key, links and properties, in no editor state. An editor
/// state never sees it unless it is put there, so changing it changes no
/// state.
pub fn clone_with_properties(latest_node: &NodeRef) -> Rc<dyn LexicalNode> {
    Rc::from(latest_node.get_latest().clone_node())
}

/// `$cloneWithPropertiesEphemeral(latestNode)`: a clone that can't join an
/// editor state, for changing in place while serializing a slice. A clone
/// from [`clone_with_properties`] is already outside every editor state, so
/// it is the same clone.
pub fn clone_with_properties_ephemeral(latest_node: &NodeRef) -> Rc<dyn LexicalNode> {
    clone_with_properties(latest_node)
}

/// `$splitNode(node, offset)`: splits the element at its child `offset`
/// and each ancestor up to the nearest root or shadow root, and returns the
/// top of the left tree (`node`'s ancestor below the root, which keeps the
/// children before the offset) and of the right tree (a copy, which gets
/// the rest). The child at `offset` moves into the right tree; its ancestors
/// are copied. An `offset` past the end splits after the last child. JS's
/// return type lets the left be `null`; it never is.
pub fn split_node(
    node: &NodeRef,
    offset: usize,
) -> Result<(Option<NodeRef>, NodeRef), LexicalError> {
    let start_node = node
        .get_child_at_index(offset)
        .unwrap_or_else(|| node.clone());
    invariant!(
        !is_root_or_shadow_root(node),
        "Can not call $splitNode() on root element"
    );
    fn recurse(
        current_node: &NodeRef,
        start_node: &NodeRef,
    ) -> Result<(NodeRef, NodeRef, NodeRef), LexicalError> {
        let parent = current_node.get_parent_or_throw()?;
        let is_parent_root = is_root_or_shadow_root(&parent);
        // The node we start split from (leaf) is moved, but its recursive
        // parents are copied to create separate tree
        let node_to_move = if current_node == start_node && !is_parent_root {
            current_node.clone()
        } else {
            copy_node(current_node)
        };
        if is_parent_root {
            invariant!(
                current_node.is_element() && node_to_move.is_element(),
                "Children of a root must be ElementNode"
            );
            current_node.insert_after(&node_to_move, true)?;
            Ok((current_node.clone(), node_to_move.clone(), node_to_move))
        } else {
            let (left_tree, right_tree, new_parent) = recurse(&parent, start_node)?;
            let mut nodes = vec![node_to_move.clone()];
            nodes.extend(current_node.get_next_siblings());
            new_parent.append(&nodes)?;
            Ok((left_tree, right_tree, node_to_move))
        }
    }
    let (left_tree, right_tree, _) = recurse(&start_node, &start_node)?;
    Ok((Some(left_tree), right_tree))
}

/// `$setNodeKey(node, null)`: gives a new node a key, puts it in the active
/// editor state and marks it dirty.
pub(crate) fn set_node_key(mut node: Box<dyn LexicalNode>) -> NodeRef {
    error_on_read_only();
    error_on_infinite_transforms();
    let key = generate_random_key();
    let editor_state = get_active_editor_state();
    error_on_frozen_node_map(&editor_state, "set");
    node.base_mut().key = key.clone();
    node.base_mut().written_in = editor_state.0.id;
    let is_element = node.element().is_some();
    editor_state
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
        // A slotted node's up-link is its slot host: an edit in a slot
        // dirties the host too.
        let parent = node
            .base()
            .parent
            .clone()
            .or_else(|| crate::slot::slot_host_key_field(&**node).cloned());
        (parent, node.element().is_some())
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
            next_parent = node
                .base()
                .parent
                .clone()
                .or_else(|| crate::slot::slot_host_key_field(&**node).cloned());
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

/// `handleDEVOnlyPendingUpdateGuarantees(editorState)`: a committed state's
/// node map is frozen, so an update that goes on after a nested `read`
/// committed it can't change it.
fn error_on_frozen_node_map(editor_state: &EditorState, method: &str) {
    if editor_state.0.read_only.get() {
        panic!("Cannot call {method}() on a frozen Lexical node map");
    }
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
        error_on_frozen_node_map(&editor_state, "set");
        *node = Rc::from(node.clone_node());
    }
    // This version is the one of this update (JS's writable clone).
    Rc::get_mut(node).expect("unshared").base_mut().written_in = editor_state.0.id;
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

/// `getTextDirection(text)`: the direction of the first strongly
/// directional UTF-16 code unit in `text` (`RTL_REGEX`, then `LTR_REGEX`:
/// `^[^LTR]*[RTL]` and `^[^RTL]*[LTR]`), or `None` when there is none.
pub fn get_text_direction(text: &str) -> Option<Direction> {
    let is_in =
        |ranges: &[(u16, u16)], unit: u16| ranges.iter().any(|&(a, b)| a <= unit && unit <= b);
    text.encode_utf16().find_map(|unit| {
        if is_in(RTL, unit) {
            Some(Direction::Rtl)
        } else if is_in(LTR, unit) {
            Some(Direction::Ltr)
        } else {
            None
        }
    })
}

/// `$setCompositionKey(compositionKey)`: the node an input method is
/// composing in, or none. The previous and the new composition nodes are
/// made writable, so they reconcile.
pub fn set_composition_key(composition_key: Option<&str>) {
    error_on_read_only();
    let editor = get_active_editor().expect("an active editor");
    let previous_composition_key = editor.composition_key();
    if composition_key != previous_composition_key.as_deref() {
        *editor.0.composition_key.borrow_mut() = composition_key.map(str::to_owned);
        *editor.0.composition_merge_held.borrow_mut() = None;
        if let Some(previous) = previous_composition_key
            && let Some(node) = get_node_by_key(&previous)
        {
            get_writable(node.get_key());
        }
        if let Some(key) = composition_key
            && let Some(node) = get_node_by_key(key)
        {
            get_writable(node.get_key());
        }
    }
}

/// `$getCompositionKey()`: the node an input method is composing in;
/// always none in a read.
pub fn get_composition_key() -> Option<NodeKey> {
    if crate::updates::is_currently_read_only_mode() {
        return None;
    }
    get_active_editor()?.composition_key()
}

/// `KeyboardEventModifiers`: a `KeyboardEvent`, or anything with its `key`,
/// `code` and modifier fields.
pub trait KeyboardEventModifiers {
    fn key(&self) -> &str;
    fn code(&self) -> &str;
    fn alt_key(&self) -> bool;
    fn ctrl_key(&self) -> bool;
    fn meta_key(&self) -> bool;
    fn shift_key(&self) -> bool;
}

/// The expected state of one modifier in a [`KeyboardEventModifierMask`]:
/// JS's `true`, `false` (or omitted) and `'any'`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModifierExpectation {
    Pressed,
    #[default]
    Released,
    Any,
}

impl ModifierExpectation {
    fn matches(self, pressed: bool) -> bool {
        match self {
            ModifierExpectation::Any => true,
            ModifierExpectation::Pressed => pressed,
            ModifierExpectation::Released => !pressed,
        }
    }
}

impl From<bool> for ModifierExpectation {
    fn from(pressed: bool) -> Self {
        if pressed {
            ModifierExpectation::Pressed
        } else {
            ModifierExpectation::Released
        }
    }
}

/// `KeyboardEventControlOrOther`'s `CONTROL_OR_OTHER_KEY`: the modifier
/// that stands in for Control on the other platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlOrOther {
    MetaKey,
    AltKey,
}

/// `KeyboardEventModifierMask` (with `KeyboardEventControlOrOther`): the
/// expected state of each modifier; none pressed by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyboardEventModifierMask {
    pub alt_key: ModifierExpectation,
    pub ctrl_key: ModifierExpectation,
    pub meta_key: ModifierExpectation,
    pub shift_key: ModifierExpectation,
    /// `[CONTROL_OR_OTHER_KEY]`: set on `CONTROL_OR_META` (`MetaKey`) and
    /// `CONTROL_OR_ALT` (`AltKey`).
    pub control_or_other: Option<ControlOrOther>,
}

impl KeyboardEventModifierMask {
    fn other(&mut self, other: ControlOrOther) -> &mut ModifierExpectation {
        match other {
            ControlOrOther::MetaKey => &mut self.meta_key,
            ControlOrOther::AltKey => &mut self.alt_key,
        }
    }
}

/// `keyboardEventMaskForPlatform(mask, isApple)` (`@internal`): the mask as
/// it is on the platform `is_apple` names, when it is a control-or-other
/// mask made for the other one.
pub fn keyboard_event_mask_for_platform(
    mask: KeyboardEventModifierMask,
    is_apple: bool,
) -> KeyboardEventModifierMask {
    match mask.control_or_other {
        Some(other) if is_apple != crate::environment::is_apple() => {
            let mut result = mask;
            let mut source = mask;
            result.ctrl_key = *source.other(other);
            *result.other(other) = mask.ctrl_key;
            result
        }
        _ => mask,
    }
}

/// `isModifierMatch(event, mask)`: whether the event's modifiers are as the
/// mask expects.
pub fn is_modifier_match(
    event: &impl KeyboardEventModifiers,
    mask: &KeyboardEventModifierMask,
) -> bool {
    mask.alt_key.matches(event.alt_key())
        && mask.ctrl_key.matches(event.ctrl_key())
        && mask.shift_key.matches(event.shift_key())
        && mask.meta_key.matches(event.meta_key())
}

/// `isExactShortcutMatch(event, expectedKey, mask)`: whether the event is
/// `expected_key` (case-insensitive) with exactly the modifiers of `mask`,
/// falling back to `event.code` for a single-character key on a non-Latin
/// layout.
pub fn is_exact_shortcut_match(
    event: &impl KeyboardEventModifiers,
    expected_key: &str,
    mask: &KeyboardEventModifierMask,
) -> bool {
    if !is_modifier_match(event, mask) {
        return false;
    }
    let key = event.key();
    if key.to_lowercase() == expected_key.to_lowercase() {
        // Special keys (Enter, Tab, ArrowUp...), and keys of an
        // English-based layout.
        return true;
    }
    if crate::utf16::len(expected_key) > 1 {
        // A special key on another layout must not match by `event.code`.
        return false;
    }
    if crate::utf16::len(key) == 1 && key.encode_utf16().next().is_some_and(|c| c <= 127) {
        // An ASCII key must not match by `event.code`, which would break
        // remapped layouts (Dvorak...).
        return false;
    }
    let code = event.code();
    // Fallback for number keys
    if code.starts_with("Digit")
        && expected_key.len() == 1
        && expected_key.as_bytes()[0].is_ascii_digit()
    {
        return code == format!("Digit{expected_key}");
    }
    // Keys of a layout whose `event.key` isn't ASCII match by `event.code`.
    code == format!("Key{}", expected_key.to_uppercase())
}

/// `isModifier(event)`: whether any modifier is down.
pub fn is_modifier(event: &impl KeyboardEventModifiers) -> bool {
    event.ctrl_key() || event.shift_key() || event.alt_key() || event.meta_key()
}

/// `isBackspace(event)`.
pub fn is_backspace(event: &impl KeyboardEventModifiers) -> bool {
    event.key() == "Backspace"
}

/// `isEscape(event)`.
pub fn is_escape(event: &impl KeyboardEventModifiers) -> bool {
    event.key() == "Escape"
}

/// `isDelete(event)`.
pub fn is_delete(event: &impl KeyboardEventModifiers) -> bool {
    event.key() == "Delete"
}

/// `isSelectAll(event)`: ⌘A on Apple platforms, Ctrl+A elsewhere.
pub fn is_select_all(event: &impl KeyboardEventModifiers) -> bool {
    is_exact_shortcut_match(event, "a", &crate::keyboard_shortcuts::control_or_meta())
}
