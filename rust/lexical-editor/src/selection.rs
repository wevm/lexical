//! `@lexical/selection` (the `selection` feature): what of it the ported
//! packages use so far, and `$setBlocksType`.
//!
//! From `packages/lexical-selection/src/range-selection.ts`.

use crate::caret::{CaretDirection, RootMode, flip_direction};
use crate::caret_utils::{caret_from_point, is_extendable_text_point_caret};
use crate::lexical_selection::get_selection;
use crate::lexical_utils::is_block;
use crate::selection_insert_nodes::{find_matching_parent, is_block_container};
use crate::{LexicalError, NodeRef, Point, PointType, SelectionRef, Which};

/// `$copyBlockFormatIndent(srcNode, destNode)`: gives `dest_node` the
/// alignment and the indent of `src_node`, setting each only where they
/// differ. What [`set_blocks_type`] does by default to the block it makes.
pub fn copy_block_format_indent(
    src_node: &NodeRef,
    dest_node: &NodeRef,
) -> Result<(), LexicalError> {
    let format = src_node.element_data().get_format_type();
    let indent = src_node.get_indent()?;
    if format != dest_node.element_data().get_format_type() {
        dest_node.set_element_format(format);
    }
    if indent != dest_node.get_indent()? {
        dest_node.set_indent(indent)?;
    }
    Ok(())
}

/// `$isAtEdgeOfElement(point, element, direction)`: whether the collapsed
/// `point` has nothing before it (`Previous`) or after it (`Next`) inside
/// `element`, at every level from its node up to `element`.
pub fn is_at_edge_of_element(
    point: &Point,
    element: &NodeRef,
    direction: CaretDirection,
) -> Result<bool, LexicalError> {
    // An extendable TextPointCaret has text remaining in `direction`, so the
    // point is in the middle of a TextNode rather than at the element edge.
    let mut caret = Some(caret_from_point(point, direction)?);
    if caret.as_ref().is_some_and(is_extendable_text_point_caret) {
        return Ok(false);
    }
    // Walk up towards element: the point is at the edge only when nothing
    // precedes it in `direction` at every level up to element.
    while let Some(c) = caret {
        let Some(parent) = c.get_parent_at_caret() else {
            return Ok(false);
        };
        if c.get_node_at_caret().is_some() {
            return Ok(false);
        }
        if parent.get_key() == element.get_key() {
            return Ok(true);
        }
        caret = c.get_parent_caret(RootMode::Root);
    }
    Ok(false)
}

/// `$isPointAtBlockEdge(point, block, direction)`: like
/// [`is_at_edge_of_element`], except that a point in an empty element is
/// never at the edge: the selection's intent is then the entire block.
fn is_point_at_block_edge(
    point: &Point,
    block: &NodeRef,
    direction: CaretDirection,
) -> Result<bool, LexicalError> {
    let node = point.get_node()?;
    if node.is_element() && node.is_empty() {
        return Ok(false);
    }
    is_at_edge_of_element(point, block, direction)
}

/// `$afterCreateElement(prevNodeSrc, newNodeDest)` of [`set_blocks_type`]:
/// updates the new block from the one it replaces.
pub type AfterCreateElement = dyn Fn(&NodeRef, &NodeRef) -> Result<(), LexicalError>;

/// `$setBlocksType(selection, $createElement, $afterCreateElement)`:
/// replaces each block the selection touches with a new element from
/// `create_element`, moving the block's children into it. The blocks are
/// those holding the anchor and the focus, and the blocks among
/// `selection.getNodes()`, or, for a selection without points (a node
/// selection), the nearest block holding each of its nodes; a focus that
/// overshoots to the leading edge of the next block leaves that block out.
/// `after_create_element(prev_node, new_node)` runs on each new element
/// before the replacement, [`copy_block_format_indent`] when `None`, as in
/// JS. `NodeRef::replace` (and `ListItemNode`'s override) remaps the points
/// on a replaced block. Nothing happens without a selection.
///
/// Slots aren't ported, so JS's skipping of a block that is a named slot's
/// value (`$getSlotHost(prevNode) !== null`) has nothing to skip.
pub fn set_blocks_type(
    selection: Option<&SelectionRef>,
    create_element: impl Fn() -> NodeRef,
    after_create_element: Option<&AfterCreateElement>,
) -> Result<(), LexicalError> {
    let Some(selection) = selection else {
        return Ok(());
    };
    let after_create_element = after_create_element.unwrap_or(&copy_block_format_indent);
    // Selections tend to not include their containing blocks so we
    // effectively expand it here
    let anchor_and_focus = selection.range().map(|range| (range.anchor, range.focus));
    let mut skip_focus = false;
    let mut focus_block: Option<NodeRef> = None;
    // A JS Map keyed by node key: insertion-ordered, and a key set again
    // keeps its place.
    let mut block_map: Vec<NodeRef> = Vec::new();
    let mut set = |node: NodeRef| {
        if !block_map.contains(&node) {
            block_map.push(node);
        }
    };
    if let Some((anchor, focus)) = &anchor_and_focus {
        let anchor_block = find_matching_parent(&anchor.get_node()?, is_block);
        focus_block = find_matching_parent(&focus.get_node()?, is_block);
        // The focus is the moving edge of the selection, travelling in
        // `direction` (towards the end of the document for a forward
        // selection). When a selection overshoots, its focus lands at the
        // leading edge of focusBlock in that direction — the edge opposite
        // to travel — so focusBlock holds none of the selection and is
        // skipped.
        let direction = if selection.is_backward()? {
            CaretDirection::Previous
        } else {
            CaretDirection::Next
        };
        skip_focus = match &focus_block {
            Some(block) if block.is_element() && anchor_block.as_ref() != Some(block) => {
                is_point_at_block_edge(focus, block, flip_direction(direction))?
            }
            _ => false,
        };
        if let Some(block) = anchor_block.filter(NodeRef::is_element) {
            set(block);
        }
        if let Some(block) = focus_block.clone().filter(NodeRef::is_element)
            && !skip_focus
        {
            set(block);
        }
    }
    for node in selection.get_nodes()? {
        if node.is_element() && is_block(&node) {
            if skip_focus && focus_block.as_ref() == Some(&node) {
                continue;
            }
            set(node);
        } else if anchor_and_focus.is_none()
            && let Some(ancestor_block) =
                find_matching_parent(&node, is_block).filter(NodeRef::is_element)
        {
            set(ancestor_block);
        }
    }
    // Selection remapping is delegated to LexicalNode.replace (and the
    // ListItemNode.replace override): both remap an element-anchored point
    // on the replaced block to {key: replacement, offset: prevSize + offset}.
    for prev_node in block_map {
        if is_block_container(&prev_node) {
            // An empty declared container (roots and shadow roots are never
            // in the map) is not a block to convert: with a selection point
            // in it (either end, in either direction), the new block goes in
            // it, as with $insertList and $wrapNodes. The container stays,
            // so its indent and format are not copied, and the points in it
            // move into the new block (a range keeps its other end). A
            // replacement above may have made a new active selection, so its
            // points count too.
            let mut selections = vec![selection.clone()];
            if let Some(active) = get_selection()
                && !active.ptr_eq(selection)
            {
                selections.push(active);
            }
            let mut points: Vec<(SelectionRef, Which)> = Vec::new();
            for s in selections.into_iter().filter(SelectionRef::is_range) {
                for which in [Which::Anchor, Which::Focus] {
                    if s.point(which).is_some_and(|p| p.key == prev_node.get_key()) {
                        points.push((s.clone(), which));
                    }
                }
            }
            if !points.is_empty() && prev_node.is_empty() {
                let element = create_element();
                prev_node.append(std::slice::from_ref(&element))?;
                for (s, which) in points {
                    s.point_set(which, element.get_key(), 0, PointType::Element)?;
                }
            }
            continue;
        }
        let element = create_element();
        after_create_element(&prev_node, &element)?;
        prev_node.replace(&element, true)?;
    }
    Ok(())
}
