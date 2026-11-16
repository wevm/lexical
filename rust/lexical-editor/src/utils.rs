//! `@lexical/utils` (the `utils` feature): the model part of
//! `packages/lexical-utils/src/index.ts`, and what it re-exports from
//! `lexical`.
//!
//! Not ported: the DOM helpers (`markSelection`, `positionNodeOnRange`,
//! `selectionAlwaysOnDisplay`, `dedupeSelectionRects`, `calculateZoomLevel`,
//! `getScrollParent`, `mediaFileReader`, `eventFiles`, `isMimeType`, the
//! class name helpers and the DOM predicates), and `objectKlassEquals`,
//! of JS constructors the port has no counterpart for; see
//! `rust/PARITY.md`.

use crate::caret::{
    CaretDirection, NodeCaret, RootMode, SiblingCaret, get_adjacent_child_caret, get_caret_range,
    get_child_caret, get_child_caret_or_self, get_collapsed_caret_range, get_sibling_caret,
};
use crate::caret_utils::{
    SplitAtPointCaretNextOptions, caret_from_point, caret_range_from_selection,
    get_caret_range_in_direction, is_extendable_text_point_caret, normalize_caret,
    remove_text_from_caret_range, rewind_sibling_caret, set_selection_from_caret_range,
    split_at_point_caret_next,
};
use crate::editor::DirtyType;
use crate::editor_state::NodeMap;
use crate::lexical_selection::{get_selection, set_selection};
use crate::lexical_utils::get_root;
use crate::nodes::paragraph::create_paragraph_node;
use crate::selection::is_at_edge_of_element;
use crate::selection_insert_nodes::{get_previous_selection, is_block_container};
use crate::updates::get_active_editor;
use crate::{
    Editor, EditorState, LexicalError, LexicalNode, NodeRef, Point, SliceMode, StateConfig,
    Unregister, get_state, set_state, set_state_with,
};
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

/// `$findMatchingParent(startingNode, findFn)`: `starting_node` or its
/// nearest ancestor below the root for which `find_fn` is true.
pub use crate::selection_insert_nodes::find_matching_parent;
/// `$getAdjacentSiblingOrParentSiblingCaret`, `$insertNodeToNearestRootAtCaret`,
/// `$isBlockFullySelected`, `$isRootOrShadowRoot`, `$splitNode` and
/// `mergeRegister`, which `@lexical/utils` re-exports from `lexical`.
pub use crate::{
    get_adjacent_sibling_or_parent_sibling_caret, insert_node_to_nearest_root_at_caret,
    is_block_fully_selected, is_root_or_shadow_root, merge_register, split_node,
};

/// `$isEditorIsNestedEditor(editor)`: whether the editor has a parent
/// editor, as `NestedEditorExtension` (or `LexicalNestedComposer`) gives it.
pub fn is_editor_is_nested_editor(editor: &Editor) -> bool {
    editor.parent_editor().is_some()
}

/// `DFSNode`: a node of a depth first traversal, with its depth (the root's
/// is 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DFSNode {
    pub depth: usize,
    pub node: NodeRef,
}

/// `$dfs(startNode, endNode)`: the nodes from `start_node` (the root when
/// `None`) in preorder, with their depths, down to all of its descendants,
/// or up to `end_node` (inclusive, and none of its children).
pub fn dfs(start_node: Option<&NodeRef>, end_node: Option<&NodeRef>) -> Vec<DFSNode> {
    dfs_iterator(start_node, end_node).collect()
}

/// `$reverseDfs(startNode, endNode)`: the right-to-left mirror of [`dfs`].
pub fn reverse_dfs(start_node: Option<&NodeRef>, end_node: Option<&NodeRef>) -> Vec<DFSNode> {
    reverse_dfs_iterator(start_node, end_node).collect()
}

/// `$dfsIterator(startNode, endNode)`: [`dfs`], walked as it is asked for.
pub fn dfs_iterator(
    start_node: Option<&NodeRef>,
    end_node: Option<&NodeRef>,
) -> impl Iterator<Item = DFSNode> + use<> {
    dfs_caret_iterator(CaretDirection::Next, start_node, end_node)
}

/// `$reverseDfsIterator(startNode, endNode)`: [`reverse_dfs`], walked as it
/// is asked for.
pub fn reverse_dfs_iterator(
    start_node: Option<&NodeRef>,
    end_node: Option<&NodeRef>,
) -> impl Iterator<Item = DFSNode> + use<> {
    dfs_caret_iterator(CaretDirection::Previous, start_node, end_node)
}

/// `$dfsWithSlots(startNode, endNode)` (`@experimental`): [`dfs`], with
/// each slot host's slot subtrees right after the host and before its
/// children. `end_node` stops the walk before its slots too; an `end_node`
/// inside a slot subtree is never reached.
pub fn dfs_with_slots(start_node: Option<&NodeRef>, end_node: Option<&NodeRef>) -> Vec<DFSNode> {
    let mut out = Vec::new();
    for dfs_node in dfs_iterator(start_node, end_node) {
        let (node, depth) = (dfs_node.node.clone(), dfs_node.depth);
        out.push(dfs_node);
        if crate::slot::is_slot_host(&node) && end_node != Some(&node) {
            for name in crate::slot::get_slot_names(&node) {
                if let Some(slot) = crate::slot::get_slot(&node, &name) {
                    dfs_subtree(&slot, depth + 1, &mut out);
                }
            }
        }
    }
    out
}

/// `$dfsWithSlotsIterator(startNode, endNode)` (`@experimental`):
/// [`dfs_with_slots`] as an iterator (walked before the first item, unlike
/// JS's generator).
pub fn dfs_with_slots_iterator(
    start_node: Option<&NodeRef>,
    end_node: Option<&NodeRef>,
) -> impl Iterator<Item = DFSNode> + use<> {
    dfs_with_slots(start_node, end_node).into_iter()
}

/// `$dfsSubtreeIterator(node, depth)`: a slots-first preorder of a slot
/// subtree.
fn dfs_subtree(node: &NodeRef, depth: usize, out: &mut Vec<DFSNode>) {
    out.push(DFSNode {
        depth,
        node: node.clone(),
    });
    if crate::slot::is_slot_host(node) {
        for name in crate::slot::get_slot_names(node) {
            if let Some(slot) = crate::slot::get_slot(node, &name) {
                dfs_subtree(&slot, depth + 1, out);
            }
        }
    }
    for child in node.get_children() {
        dfs_subtree(&child, depth + 1, out);
    }
}

/// `$reverseDfsWithSlots(startNode, endNode)` (`@experimental`): the
/// right-to-left mirror of [`dfs_with_slots`]: a host's slot subtrees, in
/// reverse slot order, after its children.
pub fn reverse_dfs_with_slots(
    start_node: Option<&NodeRef>,
    end_node: Option<&NodeRef>,
) -> Vec<DFSNode> {
    let mut out = Vec::new();
    let mut pending: Vec<(NodeRef, usize)> = Vec::new();
    for dfs_node in reverse_dfs_iterator(start_node, end_node) {
        while pending
            .last()
            .is_some_and(|(_, depth)| dfs_node.depth <= *depth)
        {
            let (host, depth) = pending.pop().expect("a pending host");
            reverse_slots_of(&host, depth + 1, &mut out);
        }
        let (node, depth) = (dfs_node.node.clone(), dfs_node.depth);
        out.push(dfs_node);
        if crate::slot::is_slot_host(&node)
            && !crate::slot::get_slot_names(&node).is_empty()
            && end_node != Some(&node)
        {
            pending.push((node, depth));
        }
    }
    while let Some((host, depth)) = pending.pop() {
        reverse_slots_of(&host, depth + 1, &mut out);
    }
    out
}

/// `$reverseDfsWithSlotsIterator(startNode, endNode)` (`@experimental`):
/// [`reverse_dfs_with_slots`] as an iterator (walked before the first item).
pub fn reverse_dfs_with_slots_iterator(
    start_node: Option<&NodeRef>,
    end_node: Option<&NodeRef>,
) -> impl Iterator<Item = DFSNode> + use<> {
    reverse_dfs_with_slots(start_node, end_node).into_iter()
}

/// `$reverseSlotsOf(host, childDepth)`.
fn reverse_slots_of(host: &NodeRef, child_depth: usize, out: &mut Vec<DFSNode>) {
    for name in crate::slot::get_slot_names(host).iter().rev() {
        if let Some(slot) = crate::slot::get_slot(host, name) {
            reverse_dfs_subtree(&slot, child_depth, out);
        }
    }
}

/// `$reverseDfsSubtreeIterator(node, depth)`.
fn reverse_dfs_subtree(node: &NodeRef, depth: usize, out: &mut Vec<DFSNode>) {
    out.push(DFSNode {
        depth,
        node: node.clone(),
    });
    for child in node.get_children().iter().rev() {
        reverse_dfs_subtree(child, depth + 1, out);
    }
    if crate::slot::is_slot_host(node) {
        reverse_slots_of(node, depth + 1, out);
    }
}

/// `$getAdjacentCaret(caret)`: `caret.getAdjacentCaret()`, or `None`.
pub fn get_adjacent_caret(caret: Option<&NodeCaret>) -> Option<SiblingCaret> {
    caret?.get_adjacent_caret()
}

/// `$getEndCaret(startNode, direction)`.
fn get_end_caret(start_node: &NodeRef, direction: CaretDirection) -> Option<NodeCaret> {
    get_adjacent_sibling_or_parent_sibling_caret(
        &get_sibling_caret(start_node, direction),
        RootMode::Root,
    )
    .map(|(caret, _)| caret)
}

/// `$dfsCaretIterator(direction, startNode, endNode)`.
fn dfs_caret_iterator(
    direction: CaretDirection,
    start_node: Option<&NodeRef>,
    end_node: Option<&NodeRef>,
) -> impl Iterator<Item = DFSNode> + use<> {
    let start = start_node.cloned().unwrap_or_else(get_root);
    let start_caret =
        get_child_caret(&start, direction).unwrap_or_else(|| get_sibling_caret(&start, direction));
    let start_depth = get_depth(Some(&start));
    let end_caret = match end_node {
        Some(end_node) => get_adjacent_child_caret(
            get_child_caret_or_self(Some(get_sibling_caret(end_node, direction))).as_ref(),
        )
        .or_else(|| get_end_caret(end_node, direction)),
        None => get_end_caret(&start, direction),
    };
    // `map` reads the depth `step` keeps, as the JS closures share it.
    let depth = Rc::new(Cell::new(start_depth));
    let step_depth = depth.clone();
    let is_end = move |caret: &NodeCaret| {
        end_caret
            .as_ref()
            .is_some_and(|end| caret.is_same_node_caret(end))
    };
    crate::make_stepwise_iterator(
        Some(start_caret),
        |state: &Option<NodeCaret>| state.is_some(),
        move |state| {
            let state = state?;
            if is_end(&state) {
                return None;
            }
            if state.is_child_caret() {
                step_depth.set(step_depth.get() + 1);
            }
            let (caret, depth_diff) =
                get_adjacent_sibling_or_parent_sibling_caret(&state, RootMode::Root)?;
            if is_end(&caret) {
                return None;
            }
            step_depth.set(step_depth.get() + depth_diff);
            Some(caret)
        },
        move |state| DFSNode {
            depth: depth.get().max(0) as usize,
            node: state
                .as_ref()
                .expect("checked by has_next")
                .origin()
                .clone(),
        },
    )
}

/// `$getNextSiblingOrParentSibling(node)`: the node's next sibling, or else
/// the next sibling of its nearest ancestor that has one, with the change
/// in depth (0 or less).
pub fn get_next_sibling_or_parent_sibling(node: &NodeRef) -> Option<(NodeRef, isize)> {
    get_adjacent_sibling_or_parent_sibling_caret(
        &get_sibling_caret(node, CaretDirection::Next),
        RootMode::Root,
    )
    .map(|(caret, depth_diff)| (caret.origin().clone(), depth_diff))
}

/// `$getDepth(node)`: the number of ancestors of the node (the root's is
/// 0), or -1 for `None`, climbing slot hosts too.
pub fn get_depth(node: Option<&NodeRef>) -> isize {
    let mut depth = -1;
    let mut inner_node = node.cloned();
    while let Some(node) = inner_node {
        depth += 1;
        // A slotted node has no parent; climb its slot host instead.
        inner_node = node
            .get_parent()
            .or_else(|| crate::slot::get_slot_host(&node));
    }
    depth
}

/// `$getNextRightPreorderNode(startingNode)`: the node after
/// `starting_node` in a right-to-left preorder traversal (its last child,
/// or else its previous sibling, or else that of its nearest ancestor that
/// has one).
pub fn get_next_right_preorder_node(starting_node: &NodeRef) -> Option<NodeRef> {
    let start_caret = get_child_caret_or_self(Some(get_sibling_caret(
        starting_node,
        CaretDirection::Previous,
    )))
    .expect("a caret");
    get_adjacent_sibling_or_parent_sibling_caret(&start_caret, RootMode::Root)
        .map(|(caret, _)| caret.origin().clone())
}

/// `$getNearestNodeOfType(node, klass)`: `node` or its nearest ancestor that
/// is a `T`.
pub fn get_nearest_node_of_type<T: LexicalNode>(node: &NodeRef) -> Option<NodeRef> {
    let mut parent = Some(node.clone());
    while let Some(p) = parent {
        if p.get_latest().is::<T>() {
            return Some(p);
        }
        parent = p.get_parent();
    }
    None
}

/// `$getNearestBlockElementAncestorOrThrow(startNode)`: `start_node` or its
/// nearest ancestor below the root that is an element and not inline.
pub fn get_nearest_block_element_ancestor_or_throw(
    start_node: &NodeRef,
) -> Result<NodeRef, LexicalError> {
    find_matching_parent(start_node, |node| node.is_element() && !node.is_inline())
        .filter(NodeRef::is_element)
        .ok_or_else(|| {
            LexicalError::new(format!(
                "Expected node {} to have closest block element node.",
                start_node.get_key()
            ))
        })
}

/// `registerNestedElementResolver(editor, targetNode, cloneNode,
/// handleOverlap)`: a transform of the nodes of type `target_node` that
/// moves one which is a child of another of that type out of it, after
/// `handle_overlap(parent, node)`, into a clone of the parent
/// (`clone_node(parent)`) with the parent's children after it, and removes
/// the parent once empty unless it can be empty. A node nested deeper is
/// left alone, as in JS.
pub fn register_nested_element_resolver(
    editor: &Editor,
    target_node: &'static str,
    clone_node: impl Fn(&NodeRef) -> NodeRef + 'static,
    handle_overlap: impl Fn(&NodeRef, &NodeRef) -> Result<(), LexicalError> + 'static,
) -> Unregister {
    let is_target_node = move |node: &NodeRef| node.get_type() == target_node;
    // `$findMatch(node)`: the child of the nearest ancestor of the target
    // type that holds `node`, and that ancestor.
    let find_match = move |node: &NodeRef| -> Option<(NodeRef, NodeRef)> {
        // First validate we don't have any children that are of the target,
        // as we need to handle them first.
        if node.get_children().iter().any(is_target_node) {
            return None;
        }
        let mut parent_node = Some(node.clone());
        while let Some(current) = parent_node {
            parent_node = current.get_parent();
            if let Some(parent) = parent_node.as_ref().filter(|p| is_target_node(p)) {
                return Some((current, parent.clone()));
            }
        }
        None
    };
    editor.register_node_transform(target_node, move |node| {
        let Some((child, parent)) = find_match(node) else {
            return Ok(());
        };
        // Simple path, we can move child out and siblings into a new parent.
        if child == *node {
            handle_overlap(&parent, node)?;
            let next_siblings = child.get_next_siblings();
            parent.insert_after(&child, true)?;
            if !next_siblings.is_empty() {
                let new_parent = clone_node(&parent);
                child.insert_after(&new_parent, true)?;
                for sibling in &next_siblings {
                    new_parent.append(std::slice::from_ref(sibling))?;
                }
            }
            if !parent.can_be_empty() && parent.get_children_size() == 0 {
                parent.remove(false)?;
            }
        }
        // Complex path, we have a deep node that isn't a child of the
        // target parent; JS doesn't implement it either.
        Ok(())
    })
}

/// `$restoreEditorState(editor, editorState)`: makes the pending editor
/// state's nodes copies of `editor_state`'s, schedules a full reconcile,
/// and sets a copy of its selection.
pub fn restore_editor_state(editor: &Editor, editor_state: &EditorState) {
    let node_map: NodeMap = editor_state
        .0
        .node_map
        .borrow()
        .iter()
        .map(|(key, node)| (key.clone(), Rc::from(node.clone_node())))
        .collect();
    if let Some(active_editor_state) = editor.pending() {
        *active_editor_state.0.node_map.borrow_mut() = node_map;
    }
    // `$fullReconcile()`.
    if let Some(active) = get_active_editor() {
        active.0.dirty.borrow_mut().dirty_type = DirtyType::FullReconcile;
    }
    set_selection(editor_state.selection().map(|s| s.clone_selection()));
}

/// `$hasContentAfter(caret)`: whether anything follows the caret before the
/// end of its nearest block container.
fn has_content_after(caret: &NodeCaret) -> bool {
    if is_extendable_text_point_caret(caret) {
        return true;
    }
    let mut c = Some(if caret.is_text_point_caret() {
        caret.get_sibling_caret()
    } else {
        caret.clone()
    });
    while let Some(current) = c {
        if current.get_node_at_caret().is_some() {
            return true;
        }
        let parent = if current.is_child_caret() {
            Some(current.origin().clone())
        } else {
            current.origin().get_parent()
        };
        if parent.as_ref().is_some_and(is_block_container) {
            return false;
        }
        c = current.get_parent_caret(RootMode::ShadowRoot);
    }
    false
}

/// `$insertNodeToNearestRoot(node)`: inserts `node` at the selection's
/// focus (the previous selection's without one, after the last selected
/// node of a node selection, or else at the end of the root before a new
/// paragraph), splitting the blocks up to the nearest root or shadow root,
/// selects after it and returns it. Splitting at the end of a block makes
/// no empty copy of it unless nothing comes after (#5433).
pub fn insert_node_to_nearest_root(node: &NodeRef) -> Result<NodeRef, LexicalError> {
    let selection = get_selection().or_else(get_previous_selection);
    let initial_caret = match selection.as_ref().and_then(|s| s.range()) {
        Some(range) => caret_from_point(&range.focus, CaretDirection::Next)?,
        None => {
            let last_node = match &selection {
                Some(selection) => selection.get_nodes()?.pop(),
                None => None,
            };
            match last_node {
                Some(last_node) => get_sibling_caret(&last_node, CaretDirection::Next),
                None => {
                    let caret = get_child_caret(&get_root(), CaretDirection::Previous)
                        .expect("the root is an element")
                        .get_flipped()?;
                    caret.insert(&create_paragraph_node())?;
                    caret
                }
            }
        }
    };
    let options = if has_content_after(&initial_caret) {
        SplitAtPointCaretNextOptions {
            should_split: |_, edge| edge != "last",
            ..SplitAtPointCaretNextOptions::default()
        }
    } else {
        SplitAtPointCaretNextOptions::default()
    };
    let insert_caret = insert_node_to_nearest_root_at_caret(node, &initial_caret, &options)?;
    let selection_caret = match get_adjacent_child_caret(Some(&insert_caret)) {
        Some(adjacent) if adjacent.is_child_caret() => normalize_caret(&adjacent),
        _ => insert_caret,
    };
    set_selection_from_caret_range(&get_collapsed_caret_range(selection_caret))?;
    Ok(node.clone())
}

/// `$insertNodeIntoLeaf(node)`: removes the selected text and inserts `node`
/// where it was, splitting a text node but no element, and puts the
/// selection after it. A node selection inserts it as `insertNodes` does.
pub fn insert_node_into_leaf(node: &NodeRef) -> Result<(), LexicalError> {
    let Some(selection) = get_selection() else {
        return Ok(());
    };
    let Some(range) = selection.range() else {
        return selection.insert_nodes(vec![node.clone()]);
    };
    let caret_range = caret_range_from_selection(&range)?;
    let mut insert_caret = get_caret_range_in_direction(
        &remove_text_from_caret_range(&caret_range, SliceMode::RemoveEmptySlices)?,
        CaretDirection::Next,
    )?
    .anchor;
    if insert_caret.is_text_point_caret() {
        let Some(next_anchor) =
            split_at_point_caret_next(&insert_caret, &SplitAtPointCaretNextOptions::default())?
        else {
            return Ok(());
        };
        insert_caret = next_anchor;
    }
    let focus = insert_caret.get_flipped()?;
    focus.insert(node)?;
    set_selection_from_caret_range(&get_caret_range(focus.clone(), focus)?)?;
    Ok(())
}

/// `$wrapNodeInElement(node, createElementNode)`: replaces `node` with a
/// new element from `create_element_node` and appends `node` to it.
pub fn wrap_node_in_element(
    node: &NodeRef,
    create_element_node: impl FnOnce() -> NodeRef,
) -> Result<NodeRef, LexicalError> {
    let element_node = create_element_node();
    node.replace(&element_node, false)?;
    element_node.append(std::slice::from_ref(node))?;
    Ok(element_node)
}

/// `$filter(nodes, filterFn)` (deprecated upstream for `filter_map`): the
/// values `filter_fn` gives for `nodes`, without the `None`s.
pub fn filter<T>(nodes: &[NodeRef], filter_fn: impl FnMut(&NodeRef) -> Option<T>) -> Vec<T> {
    nodes.iter().filter_map(filter_fn).collect()
}

/// `$handleIndentAndOutdent(indentOrOutdent)`: applies `indent_or_outdent`
/// once to each block (non-inline element) holding a node of the range
/// selection whose `canIndent()` is true. Returns whether it handled one.
pub fn handle_indent_and_outdent(
    indent_or_outdent: impl Fn(&NodeRef) -> Result<(), LexicalError>,
) -> Result<bool, LexicalError> {
    let Some(selection) = get_selection().filter(|s| s.is_range()) else {
        return Ok(false);
    };
    let mut already_handled: HashSet<String> = HashSet::new();
    for node in selection.get_nodes()? {
        if already_handled.contains(node.get_key()) {
            continue;
        }
        let Some(parent_block) =
            find_matching_parent(&node, |parent| parent.is_element() && !parent.is_inline())
        else {
            continue;
        };
        let parent_key = parent_block.get_key().to_owned();
        if parent_block.can_indent() && !already_handled.contains(&parent_key) {
            already_handled.insert(parent_key);
            indent_or_outdent(&parent_block)?;
        }
    }
    Ok(!already_handled.is_empty())
}

/// `$insertFirst(parent, node)`: inserts `node` before the first child of
/// `parent`.
pub fn insert_first(parent: &NodeRef, node: &NodeRef) -> Result<(), LexicalError> {
    get_child_caret(parent, CaretDirection::Next)
        .expect("a parent is an element")
        .insert(node)
}

/// `$unwrapAndFilterDescendants(root, predicate)`: walks the descendants of
/// `root` last to first, and removes each that `predicate` rejects, keeping
/// in its place its own descendants that `predicate` accepts (so those end
/// up children of `root`). Returns whether it unwrapped or removed any.
pub fn unwrap_and_filter_descendants(
    root: &NodeRef,
    predicate: impl Fn(&NodeRef) -> bool,
) -> Result<bool, LexicalError> {
    unwrap_and_filter_descendants_impl(root, &predicate, None)
}

/// `onSuccess` of `$unwrapAndFilterDescendantsImpl`: where an accepted
/// descendant of a removed node goes.
type OnSuccess<'a> = &'a dyn Fn(&NodeRef) -> Result<(), LexicalError>;

/// `$unwrapAndFilterDescendantsImpl(root, predicate, onSuccess)`.
fn unwrap_and_filter_descendants_impl(
    root: &NodeRef,
    predicate: &dyn Fn(&NodeRef) -> bool,
    on_success: Option<OnSuccess<'_>>,
) -> Result<bool, LexicalError> {
    let mut did_mutate = false;
    for node in last_to_first_iterator(root) {
        if predicate(&node) {
            if let Some(on_success) = on_success {
                on_success(&node)?;
            }
            continue;
        }
        did_mutate = true;
        if node.is_element() {
            let insert_after = |child: &NodeRef| node.insert_after(child, true).map(drop);
            unwrap_and_filter_descendants_impl(
                &node,
                predicate,
                Some(on_success.unwrap_or(&insert_after)),
            )?;
        }
        node.remove(false)?;
    }
    Ok(did_mutate)
}

/// `$descendantsMatching(children, predicate)`: the nodes among `children`
/// and their descendants that `predicate` accepts and that have no accepted
/// ancestor among them, in document order. It changes nothing.
pub fn descendants_matching(
    children: &[NodeRef],
    predicate: impl Fn(&NodeRef) -> bool,
) -> Vec<NodeRef> {
    let mut result = Vec::new();
    let mut stack: Vec<NodeRef> = children.iter().rev().cloned().collect();
    while let Some(child) = stack.pop() {
        if predicate(&child) {
            result.push(child);
        } else if child.is_element() {
            stack.extend(last_to_first_iterator(&child));
        }
    }
    result
}

/// `$firstToLastIterator(node)`: the children of `node`, first to last,
/// each next one read before the current one is yielded, so a caller may
/// remove the child it is given.
pub fn first_to_last_iterator(node: &NodeRef) -> impl Iterator<Item = NodeRef> + use<> {
    child_iterator(get_child_caret(node, CaretDirection::Next).expect("an element"))
}

/// `$lastToFirstIterator(node)`: the children of `node`, last to first, as
/// [`first_to_last_iterator`].
pub fn last_to_first_iterator(node: &NodeRef) -> impl Iterator<Item = NodeRef> + use<> {
    child_iterator(get_child_caret(node, CaretDirection::Previous).expect("an element"))
}

/// `$childIterator(startCaret)`. JS's development build checks for a cycle,
/// which only a bug in a node can make; so does this, by panicking.
fn child_iterator(start_caret: NodeCaret) -> impl Iterator<Item = NodeRef> {
    let mut seen: HashSet<String> = HashSet::new();
    crate::make_stepwise_iterator(
        start_caret.get_adjacent_caret(),
        |caret: &Option<SiblingCaret>| caret.as_ref().is_some_and(|c| c.is_sibling_caret()),
        |caret| caret.and_then(|c| c.get_adjacent_caret()),
        move |caret| {
            let origin = caret
                .as_ref()
                .expect("checked by has_next")
                .origin()
                .clone();
            let key = origin.get_key();
            assert!(
                seen.insert(key.to_owned()),
                "$childIterator: Cycle detected, node with key {key} has already been traversed"
            );
            origin
        },
    )
}

/// `$unwrapNode(node)`: replaces `node` with its children.
pub fn unwrap_node(node: &NodeRef) -> Result<(), LexicalError> {
    rewind_sibling_caret(&get_sibling_caret(node, CaretDirection::Next))?.splice(
        1,
        &node.get_children(),
        CaretDirection::Next,
    )
}

/// `StateConfigWrapper`: `$get` and `$set` bound to a state config. JS's
/// `accessors`, `makeGetterMethod` and `makeSetterMethod` build methods on
/// a JS class, which Rust nodes don't take.
pub struct StateConfigWrapper<V> {
    pub state_config: StateConfig<V>,
}

impl<V: Clone + 'static> StateConfigWrapper<V> {
    /// `$get(node)`: `$getState(node, stateConfig)`.
    pub fn get(&self, node: &NodeRef) -> Result<V, LexicalError> {
        get_state(node, &self.state_config)
    }

    /// `$set(node, value)`: `$setState(node, stateConfig, value)`.
    pub fn set(&self, node: &NodeRef, value: V) -> Result<NodeRef, LexicalError> {
        set_state(node, &self.state_config, value)
    }

    /// `$set(node, updater)`: `$setState(node, stateConfig, updater)`.
    pub fn set_with(
        &self,
        node: &NodeRef,
        updater: impl FnOnce(&V) -> V,
    ) -> Result<NodeRef, LexicalError> {
        set_state_with(node, &self.state_config, updater)
    }
}

/// `makeStateWrapper(stateConfig)` (experimental upstream).
pub fn make_state_wrapper<V>(state_config: StateConfig<V>) -> StateConfigWrapper<V> {
    StateConfigWrapper { state_config }
}

/// `$onEscapeUp($isContainerNode, event)`: with the collapsed selection at
/// the start of the nearest container (as `is_container_node` tells) that is
/// its parent's first child, inserts a paragraph before the container and
/// selects it. Returns whether it did. (JS also prevents the keyboard
/// event's default action; the port has no events.)
pub fn on_escape_up(is_container_node: impl Fn(&NodeRef) -> bool) -> Result<bool, LexicalError> {
    on_escape(is_container_node, CaretDirection::Previous)
}

/// `$onEscapeDown($isContainerNode, event)`: as [`on_escape_up`], after a
/// container that is its parent's last child, with the selection at its end.
pub fn on_escape_down(is_container_node: impl Fn(&NodeRef) -> bool) -> Result<bool, LexicalError> {
    on_escape(is_container_node, CaretDirection::Next)
}

fn on_escape(
    is_container_node: impl Fn(&NodeRef) -> bool,
    direction: CaretDirection,
) -> Result<bool, LexicalError> {
    let Some(range) = get_selection().and_then(|s| s.range()) else {
        return Ok(false);
    };
    if !range.is_collapsed() {
        return Ok(false);
    }
    let Some(container_node) = find_matching_parent(&range.anchor.get_node()?, is_container_node)
    else {
        return Ok(false);
    };
    let Some(parent) = container_node.get_parent() else {
        return Ok(false);
    };
    let up = direction == CaretDirection::Previous;
    let edge_child = if up {
        parent.get_first_child()
    } else {
        parent.get_last_child()
    };
    if edge_child.as_ref() != Some(&container_node)
        || !is_at_edge_of_element(&range.anchor, &container_node, direction)?
    {
        return Ok(false);
    }
    let paragraph = create_paragraph_node();
    if up {
        container_node.insert_before(&paragraph, true)?;
    } else {
        container_node.insert_after(&paragraph, true)?;
    }
    paragraph.select_end()?;
    Ok(true)
}

/// `$isAtStartOfNode(point, node)`: whether the collapsed `point` sits at
/// the very start of `node`'s content.
pub fn is_at_start_of_node(point: &Point, node: &NodeRef) -> Result<bool, LexicalError> {
    is_at_edge_of_element(point, node, CaretDirection::Previous)
}

/// `$isAtEndOfNode(point, node)`: whether the collapsed `point` sits at the
/// very end of `node`'s content.
pub fn is_at_end_of_node(point: &Point, node: &NodeRef) -> Result<bool, LexicalError> {
    is_at_edge_of_element(point, node, CaretDirection::Next)
}
