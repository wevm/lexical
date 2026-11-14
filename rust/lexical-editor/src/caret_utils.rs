//! `caret/LexicalCaretUtils.ts`: carets from selection points and back,
//! normalization, and removing the text and nodes of a caret range.
//!
//! Slots, NodeState and the text edit log are not ported, so the parts of
//! `$removeTextFromCaretRange` that keep a slot host, carry NodeState to a
//! replacement node or record deletions do nothing here.

use crate::LexicalError;
use crate::caret::{
    CaretDirection, CaretRange, CommonAncestorResult, NodeCaret, PointCaret, RootMode,
    SiblingCaret, TextOffset, TextPointCaretSlice, compare_point_caret_next, flip_direction,
    get_adjacent_child_caret, get_caret_in_direction, get_caret_range, get_child_caret,
    get_collapsed_caret_range, get_common_ancestor, get_sibling_caret, get_text_node_offset,
    get_text_point_caret, get_text_point_caret_at_end,
};
use crate::error::invariant;
use crate::lexical_selection::{
    Point, PointType, RangeSelection, SelectionRef, Which, create_range_selection, get_selection,
    set_selection,
};
use crate::lexical_utils::{
    get_node_by_key_or_throw, get_root, is_block, is_shadow_root_node, remove_from_parent,
    restore_empty_container_paragraph,
};
use crate::node::{NodeKey, NodeRef};
use crate::nodes::element::is_root_or_shadow_root;
use crate::nodes::text::create_text_node;
use std::collections::HashSet;

/// `$caretFromPoint(point, direction)`: the caret at a selection point.
pub fn caret_from_point(
    point: &Point,
    direction: CaretDirection,
) -> Result<PointCaret, LexicalError> {
    let node = get_node_by_key_or_throw(&point.key)?;
    if point.point_type == PointType::Text {
        invariant!(
            node.is_text(),
            "$caretFromPoint: Node with type {} and key {} that does not inherit from TextNode encountered for text point",
            node.get_type(),
            point.key
        );
        return get_text_point_caret(&node, direction, point.offset);
    }
    invariant!(
        node.is_element(),
        "$caretFromPoint: Node with type {} and key {} that does not inherit from ElementNode encountered for element point",
        node.get_type(),
        point.key
    );
    Ok(get_child_caret_at_index(&node, point.offset, direction))
}

/// `$setPointFromCaret(point, caret)`: sets the point `which` of a range
/// selection to the caret's position.
pub fn set_point_from_caret(
    selection: &SelectionRef,
    which: Which,
    caret: &PointCaret,
) -> Result<(), LexicalError> {
    let origin = caret.origin();
    let direction = caret.direction();
    let is_next = direction == CaretDirection::Next;
    if let Some(offset) = caret.offset() {
        selection.point_set(which, origin.get_key(), offset, PointType::Text)
    } else if caret.is_sibling_caret() {
        if origin.is_text() {
            let offset = get_text_node_offset(origin, direction.into(), false)?;
            selection.point_set(which, origin.get_key(), offset, PointType::Text)
        } else {
            let parent = origin.get_parent_or_throw()?;
            let index = origin
                .get_index_within_parent()
                .expect("a child of its parent");
            selection.point_set(
                which,
                parent.get_key(),
                index + usize::from(is_next),
                PointType::Element,
            )
        }
    } else {
        assert!(
            caret.is_child_caret() && origin.is_element(),
            "$setPointFromCaret: exhaustiveness check"
        );
        let offset = if is_next {
            0
        } else {
            origin.get_children_size()
        };
        selection.point_set(which, origin.get_key(), offset, PointType::Element)
    }
}

/// `$setSelectionFromCaretRange(caretRange)`: sets the editor's range
/// selection (the current one, or a new one) to the range, and returns it.
pub fn set_selection_from_caret_range(
    caret_range: &CaretRange,
) -> Result<SelectionRef, LexicalError> {
    let selection = get_selection()
        .filter(SelectionRef::is_range)
        .unwrap_or_else(create_range_selection);
    update_range_selection_from_caret_range(&selection, caret_range)?;
    set_selection(Some(selection.clone()));
    Ok(selection)
}

/// `$updateRangeSelectionFromCaretRange(selection, caretRange)`.
pub fn update_range_selection_from_caret_range(
    selection: &SelectionRef,
    caret_range: &CaretRange,
) -> Result<(), LexicalError> {
    set_point_from_caret(selection, Which::Anchor, &caret_range.anchor)?;
    set_point_from_caret(selection, Which::Focus, &caret_range.focus)
}

/// `$caretRangeFromSelection(selection)`: the carets of a range selection,
/// in the `previous` direction when the focus is before the anchor.
pub fn caret_range_from_selection(selection: &RangeSelection) -> Result<CaretRange, LexicalError> {
    let anchor_caret = caret_from_point(&selection.anchor, CaretDirection::Next)?;
    let focus_caret = caret_from_point(&selection.focus, CaretDirection::Next)?;
    let direction = if compare_point_caret_next(&anchor_caret, &focus_caret)?.is_le() {
        CaretDirection::Next
    } else {
        CaretDirection::Previous
    };
    get_caret_range(
        get_caret_in_direction(&anchor_caret, direction)?,
        get_caret_in_direction(&focus_caret, direction)?,
    )
}

/// `$rewindSiblingCaret(caret)`: the caret, in the same direction, whose
/// adjacent caret is `caret`.
pub fn rewind_sibling_caret(caret: &SiblingCaret) -> Result<NodeCaret, LexicalError> {
    let direction = caret.direction();
    let origin = caret.origin();
    // Rotate the direction around the origin and get the adjacent node
    let rewind_origin = get_sibling_caret(origin, flip_direction(direction)).get_node_at_caret();
    Ok(match rewind_origin {
        Some(rewind_origin) => get_sibling_caret(&rewind_origin, direction),
        None => get_child_caret(&origin.get_parent_or_throw()?, direction)
            .expect("a parent is an element"),
    })
}

/// `$getAnchorCandidates(anchor, rootMode)`: the anchor, the caret pointing
/// to it, and those pointing to each of its parents.
fn get_anchor_candidates(
    anchor: &PointCaret,
    root_mode: RootMode,
) -> Result<Vec<PointCaret>, LexicalError> {
    // These candidates will be the anchor itself, the pointer to the anchor
    // (if different), and then any parents of that
    let mut carets = vec![anchor.clone()];
    let mut parent = if anchor.is_child_caret() {
        anchor.get_parent_caret(root_mode)
    } else {
        Some(anchor.get_sibling_caret())
    };
    while let Some(p) = parent {
        carets.push(rewind_sibling_caret(&p)?);
        parent = p.get_parent_caret(root_mode);
    }
    Ok(carets)
}

/// `$isCaretAttached(caret)`.
fn is_caret_attached(caret: &PointCaret) -> bool {
    caret.origin().is_attached()
}

/// The `sliceMode` of [`remove_text_from_caret_range`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SliceMode {
    /// `'removeEmptySlices'`: a text node whose text is all removed is
    /// removed.
    #[default]
    RemoveEmptySlices,
    /// `'preserveEmptyTextSliceCaret'`: the first such text node is kept,
    /// empty, for an insertion.
    PreserveEmptyTextSliceCaret,
}

/// `$removeTextFromCaretRange(initialRange, sliceMode)`: removes the text
/// and nodes in the range, merging the rest of the later block into the
/// earlier one when the range spans blocks. Returns the collapsed range
/// where the removal was, in the initial range's direction.
pub fn remove_text_from_caret_range(
    initial_range: &CaretRange,
    slice_mode: SliceMode,
) -> Result<CaretRange, LexicalError> {
    if initial_range.is_collapsed() {
        return Ok(initial_range.clone());
    }
    // Always process removals in document order
    let root_mode = RootMode::Root;
    let next_direction = CaretDirection::Next;
    let mut slice_state = slice_mode;
    let range = get_caret_range_in_direction(initial_range, next_direction)?;

    // The nearest root or shadow root the removal happens inside, captured
    // before anything is detached; see the repair below.
    let mut range_container = Some(range.anchor.origin().clone());
    while let Some(container) = range_container
        .as_ref()
        .filter(|c| !is_root_or_shadow_root(&*c.get_latest()))
    {
        range_container = container.get_parent();
    }
    // A child of the container, sampled while it still has one: it is what
    // tells $restoreEmptyContainerParagraph whether this container holds
    // blocks (a table cell) or structure (a table's rows).
    let range_container_child = range_container
        .as_ref()
        .filter(|c| c.is_element())
        .and_then(NodeRef::get_first_child);

    let mut anchor_candidates = get_anchor_candidates(&range.anchor, root_mode)?;
    let mut focus_candidates = get_anchor_candidates(&range.focus.get_flipped()?, root_mode)?;

    // Mark the start of each ElementNode
    let mut seen_start: HashSet<NodeKey> = HashSet::new();
    // Queue removals to avoid mutating the tree during iteration
    let mut removed_nodes: Vec<NodeRef> = Vec::new();
    for caret in range.iter_node_carets(root_mode) {
        if caret.is_child_caret() {
            seen_start.insert(caret.origin().get_key().to_owned());
        } else if caret.is_sibling_caret() {
            let origin = caret.origin();
            if !origin.is_element() || seen_start.contains(origin.get_key()) {
                removed_nodes.push(origin.clone());
            }
        }
    }
    // Selection is rebuilt from anchor/focus candidates below, so detach
    // without restoring it for every removed node.
    let mut removed_parents: Vec<NodeRef> = Vec::new();
    for node in &removed_nodes {
        // Track parents not in seenStart — those in seenStart are traversal
        // boundaries handled by block-merge logic below.
        if let Some(parent) = node.get_parent()
            && !seen_start.contains(parent.get_key())
            && !removed_parents.contains(&parent)
        {
            removed_parents.push(parent);
        }
        // `$recordDeleted(node)`: the text edit log isn't ported.
        remove_from_parent(node);
    }
    // Remove inline wrappers (canBeEmpty=false) that became empty
    for parent in &removed_parents {
        if !parent.can_be_empty()
            && !is_root_or_shadow_root(&*parent.get_latest())
            && parent.is_empty()
            && parent.is_attached()
        {
            parent.remove(false)?;
        }
    }

    // Splice text at the anchor and/or origin.
    // If the text is entirely selected then it is removed (unless it is the
    // first slice and sliceMode is preserveEmptyTextSliceCaret).
    // If it's a token with a non-empty selection then it is removed.
    // Segmented nodes will be copied to a plain text node with the same
    // format and style and set to normal mode.
    for slice in range.get_text_slices().into_iter().flatten() {
        let origin = slice.caret.origin().clone();
        let content_size = origin.get_text_content_size();
        let caret_before = rewind_sibling_caret(&get_sibling_caret(&origin, next_direction))?;
        let mode = origin.text_data().get_mode();
        if (slice.distance.unsigned_abs() == content_size
            && slice_state == SliceMode::RemoveEmptySlices)
            || (mode == "token" && slice.distance != 0)
        {
            // anchorCandidates[1] should still be valid, it is caretBefore
            caret_before.remove()?;
        } else if slice.distance != 0 {
            slice_state = SliceMode::RemoveEmptySlices;
            let mut next_caret = slice.remove_text_slice()?;
            let slice_origin = slice.caret.origin();
            if mode == "segmented" {
                let src = next_caret.origin().clone();
                let data = src.text_data();
                let plain_text_node = create_text_node(data.get_text());
                plain_text_node.set_style(data.get_style());
                plain_text_node.set_format(data.get_format());
                plain_text_node.set_detail(data.get_detail())?;
                // The characters left keep their NodeState, once NodeState
                // is ported (`$carryNodeState`).
                caret_before.replace_or_insert(&plain_text_node, false)?;
                next_caret = get_text_point_caret(
                    &plain_text_node,
                    next_direction,
                    next_caret.offset().expect("a text point caret"),
                )?;
            }
            if slice_origin == anchor_candidates[0].origin() {
                anchor_candidates[0] = next_caret.clone();
            }
            if slice_origin == focus_candidates[0].origin() {
                focus_candidates[0] = next_caret.get_flipped()?;
            }
        }
    }

    // Find the deepest anchor and focus candidates that are still attached
    let anchor_candidate = get_attached_caret(anchor_candidates.iter());
    let focus_candidate = get_attached_caret(focus_candidates.iter());

    // Merge blocks if necessary
    let merge_targets = get_block_merge_targets(
        anchor_candidate.as_ref(),
        focus_candidate.as_ref(),
        &seen_start,
    );
    if let Some((anchor_block, focus_block)) = merge_targets {
        // always merge blocks later in the document with blocks earlier in
        // the document
        get_child_caret(&anchor_block, CaretDirection::Previous)
            .expect("a block is an element")
            .splice(0, &focus_block.get_children(), CaretDirection::Next)?;
        // remove empty parent node even if parent node is canBeEmpty
        let mut parent = focus_block.get_parent();
        focus_block.remove(true)?;
        while let Some(p) = parent.filter(NodeRef::is_empty) {
            parent = p.get_parent();
            p.remove(true)?;
        }
    } else if let Some(focus_candidate) = &focus_candidate {
        let focus_block = get_block_from_caret(focus_candidate);
        let focus_block_parent = focus_block.as_ref().and_then(NodeRef::get_parent);
        let topmost_shadow_root = focus_block
            .as_ref()
            .and_then(|b| b.get_parents().into_iter().rev().find(is_shadow_root_node));
        if let (Some(focus_block), Some(focus_block_parent)) = (&focus_block, &focus_block_parent)
            && !focus_block_parent.is_root()
            && focus_block.is_empty()
            && seen_start.contains(focus_block.get_key())
            // A slot host keeps its block, once slots are ported.
            && topmost_shadow_root.is_none_or(|r| seen_start.contains(r.get_key()))
        {
            focus_block.remove(true)?;
            let mut parent = Some(focus_block_parent.clone());
            while let Some(p) = parent.filter(|p| !p.is_root() && p.is_empty()) {
                let grandparent = p.get_parent();
                if let Some(g) = &grandparent
                    && g.is_root()
                    && g.get_children_size() <= 1
                    // Only keep the root's last child when it is valid while
                    // empty. One that declares `canBeEmpty()` false is
                    // removed like any other emptied ancestor, and the empty
                    // root is repaired below.
                    && p.can_be_empty()
                {
                    break;
                }
                parent = grandparent;
                p.remove(true)?;
            }
        }
    }

    // A range that covers every child of its container (a select-all over a
    // document that is a single shadow root, or over everything inside one)
    // removes them all and leaves nothing to put a caret in. Restore the
    // empty paragraph the container would otherwise be missing, so that
    // deleting all of a container's content behaves like deleting a document
    // made of ordinary paragraphs. The range is always attached (the
    // candidate walk above throws otherwise), so an empty container here is
    // always this call's doing. `rangeContainer` is itself removed when the
    // range covered it — the empty-ancestor walks above climb out of it — in
    // which case the root is what needs the paragraph.
    if restore_empty_container_paragraph(range_container.as_ref(), range_container_child.as_ref())?
        .is_none()
        && range_container.as_ref().is_some_and(|c| !c.is_attached())
    {
        restore_empty_container_paragraph(Some(&get_root()), None)?;
    }

    // note this caret can be in either direction
    let best_candidate = get_attached_caret(
        anchor_candidate
            .iter()
            .chain(&focus_candidate)
            .chain(&anchor_candidates)
            .chain(&focus_candidates),
    );
    if let Some(best_candidate) = best_candidate {
        let anchor = get_caret_in_direction(&best_candidate, initial_range.direction())?;
        return Ok(get_collapsed_caret_range(anchor));
    }
    let keys: Vec<String> = anchor_candidates
        .iter()
        .map(|c| format!("\"{}\"", c.origin().get_key()))
        .collect();
    Err(LexicalError::new(format!(
        "$removeTextFromCaretRange: selection was lost, could not find a new anchor given candidates with keys: [{}]",
        keys.join(",")
    )))
}

/// `$getAttachedCaret(candidates)`: the first candidate still attached,
/// normalized.
fn get_attached_caret<'a>(
    mut candidates: impl Iterator<Item = &'a PointCaret>,
) -> Option<PointCaret> {
    candidates
        .find(|c| is_caret_attached(c))
        .map(normalize_caret)
}

/// `$getBlockFromCaret(caret)`.
fn get_block_from_caret(caret: &PointCaret) -> Option<NodeRef> {
    if caret.is_child_caret() {
        let origin = caret.origin();
        is_block(origin).then(|| origin.clone())
    } else {
        caret.get_parent_at_caret().filter(is_block)
    }
}

/// `$getBlockMergeTargets(anchor, focus, seenStart)`: the blocks closest to
/// the common ancestor of the anchor's and the focus's parents, if they are
/// distinct and no shadow root lies between them and their carets.
fn get_block_merge_targets(
    anchor: Option<&PointCaret>,
    focus: Option<&PointCaret>,
    seen_start: &HashSet<NodeKey>,
) -> Option<(NodeRef, NodeRef)> {
    let anchor_parent = anchor?.get_parent_at_caret()?;
    let focus_parent = focus?.get_parent_at_caret()?;
    let common = get_common_ancestor(&anchor_parent, &focus_parent)?;
    if !matches!(common, CommonAncestorResult::Branch { .. }) {
        return None;
    }
    let get_block = |parent: &NodeRef, selected_only: bool| -> Option<NodeRef> {
        let mut block = None;
        // Ascend to the common ancestor, retaining the outermost eligible
        // block and rejecting any branch that crosses a shadow root.
        let mut node = Some(parent.clone());
        while let Some(n) = node.filter(|n| n != common.common_ancestor()) {
            if is_root_or_shadow_root(&*n.get_latest()) {
                return None;
            }
            if (!selected_only || seen_start.contains(n.get_key())) && is_block(&n) {
                block = Some(n.clone());
            }
            node = n.get_parent();
        }
        block
    };
    let anchor_block = get_block(&anchor_parent, false)?;
    let focus_block = get_block(&focus_parent, true)?;
    // Removing a host must not silently discard its slots during a block
    // merge, once slots are ported.
    Some((anchor_block, focus_block))
}

/// `$getDeepestChildOrSelf(initialCaret)`: the deepest child caret under
/// the caret's origin, or the caret.
fn get_deepest_child_or_self(initial_caret: PointCaret) -> PointCaret {
    let mut caret = initial_caret;
    while caret.is_child_caret() {
        match get_adjacent_child_caret(Some(&caret)) {
            Some(adjacent) if adjacent.is_child_caret() => caret = adjacent,
            _ => break,
        }
    }
    caret
}

/// `$normalizeCaret(initialCaret)`: the deepest equivalent caret, a text
/// point caret when there is text at the caret.
pub fn normalize_caret(initial_caret: &PointCaret) -> PointCaret {
    let caret = get_deepest_child_or_self(initial_caret.get_latest());
    let direction = caret.direction();
    if caret.origin().is_text() {
        return if caret.is_text_point_caret() {
            caret
        } else {
            get_text_point_caret_at_end(caret.origin(), direction, direction)
        };
    }
    match caret.get_adjacent_caret() {
        Some(adj) if adj.is_sibling_caret() && adj.origin().is_text() => {
            get_text_point_caret_at_end(adj.origin(), direction, flip_direction(direction))
        }
        _ => caret,
    }
}

/// `$isExtendableTextPointCaret(caret)`: a text point caret whose offset
/// can move further in its direction without leaving the node.
pub fn is_extendable_text_point_caret(caret: &PointCaret) -> bool {
    caret.offset().is_some_and(|offset| {
        get_text_node_offset(caret.origin(), TextOffset::End(caret.direction()), false)
            .is_ok_and(|end| offset != end)
    })
}

/// `$getCaretRangeInDirection(range, direction)`: the range, or the same
/// part of the document iterated the other way.
pub fn get_caret_range_in_direction(
    range: &CaretRange,
    direction: CaretDirection,
) -> Result<CaretRange, LexicalError> {
    if range.direction() == direction {
        return Ok(range.clone());
    }
    get_caret_range(
        // focus and anchor get flipped here
        get_caret_in_direction(&range.focus, direction)?,
        get_caret_in_direction(&range.anchor, direction)?,
    )
}

/// `$getChildCaretAtIndex(parent, index, direction)`: a caret pointing to
/// the child at `index`, or the last caret in `parent` past the end.
pub fn get_child_caret_at_index(
    parent: &NodeRef,
    index: usize,
    direction: CaretDirection,
) -> NodeCaret {
    let size = parent.get_children_size();
    let offset = index.min(size);
    let origin = match direction {
        CaretDirection::Next => offset.checked_sub(1),
        CaretDirection::Previous => Some(offset),
    }
    .filter(|&i| i < size)
    .and_then(|i| parent.get_child_at_index(i));
    match origin {
        Some(origin) => get_sibling_caret(&origin, direction),
        None => get_child_caret(parent, direction).expect("a parent is an element"),
    }
}

/// `$getAdjacentSiblingOrParentSiblingCaret(startCaret, rootMode)`: the
/// next caret in a depth first walk that doesn't descend into the start,
/// with the change in depth.
pub fn get_adjacent_sibling_or_parent_sibling_caret(
    start_caret: &NodeCaret,
    root_mode: RootMode,
) -> Option<(NodeCaret, isize)> {
    let mut depth_diff = 0;
    let mut caret = start_caret.clone();
    let mut next_caret = get_adjacent_child_caret(Some(&caret));
    while next_caret.is_none() {
        depth_diff -= 1;
        caret = caret.get_parent_caret(root_mode)?;
        next_caret = get_adjacent_child_caret(Some(&caret));
    }
    next_caret.map(|c| (c, depth_diff))
}

/// `$getTextPointCaretSliceForNode(slices, node)`: the slice of `node`
/// among a caret range's text slices.
pub fn get_text_point_caret_slice_for_node<'a>(
    slices: &'a [Option<TextPointCaretSlice>],
    node: &NodeRef,
) -> Option<&'a TextPointCaretSlice> {
    slices
        .iter()
        .flatten()
        .find(|slice| slice.caret.origin() == node)
}
