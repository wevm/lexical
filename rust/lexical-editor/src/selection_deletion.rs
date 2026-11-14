//! `deleteCharacter`, `deleteWord`, `deleteLine` and `modify` from
//! `LexicalSelection.ts`.
//!
//! JS measures how far a deletion or a caret movement reaches inside text
//! with the DOM's `Selection.modify` (`$extendSelectionForDeletion`,
//! `modify`), after `$modifySelectionAroundDecoratorsAndBlocks` has resolved
//! the moves that cross blocks in the model. That measurement starts with
//! `getWindow(editor)`, which throws `window object not found` for an editor
//! without a root element, so a headless JS editor fails the update whenever
//! a deletion or `modify` needs it: within text, at a text node edge with a
//! sibling in the deletion direction, and at the edges of the document.
//! Only the model paths (merging blocks, removing a line break or an empty
//! block, removing an expanded selection) succeed.
//!
//! An editor with a [`SelectionMovement`]
//! ([`Editor::set_selection_movement`](crate::Editor::set_selection_movement))
//! stands in for `Selection.modify`: it says where the caret lands, and the
//! rest follows JS from there. The DOM points JS resolves back into the model
//! (`applyDOMRange`) are model points here already, so only the model side
//! of that resolution is ported. Without one, this port throws where
//! headless JS does.
//!
//! Slots and shadow roots other than the root are not ported, so the
//! branches for them are no-ops (each is noted where JS has it), as are the
//! parts that read a decorator's DOM.

use crate::LexicalError;
use crate::caret::{
    CaretDirection, NodeCaret, PointCaret, RootMode, compare_point_caret_next,
    extend_caret_to_range, get_caret_in_direction, get_caret_range, get_child_caret,
    get_sibling_caret,
};
use crate::caret_utils::{
    caret_from_point, caret_range_from_selection, get_caret_range_in_direction,
    is_extendable_text_point_caret, normalize_caret, rewind_sibling_caret, set_point_from_caret,
    set_selection_from_caret_range, update_range_selection_from_caret_range,
};
use crate::error::invariant;
use crate::lexical_selection::{
    Point, PointType, RangeSelection, SelectionRef, Which, create_node_selection, get_selection,
    set_selection,
};
use crate::lexical_utils::{
    get_root, is_block, is_shadow_root_node, restore_empty_container_paragraph,
};
use crate::node::NodeRef;
use crate::nodes::element::is_root_or_shadow_root;
use crate::nodes::line_break::LineBreakNode;
use crate::nodes::paragraph::create_paragraph_node;
use crate::selection_insert_nodes::{get_previous_selection, normalize_selection};
use crate::updates::get_active_editor;
use crate::utf16;
use std::rc::Rc;

/// `'character'`, `'word'`, `'line'` or `'lineboundary'`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionGranularity {
    Character,
    Word,
    Line,
    LineBoundary,
}

/// `'move'` or `'extend'`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionAlter {
    Move,
    Extend,
}

/// Where a caret lands when it moves one unit, which only text layout knows
/// (the DOM's `Selection.modify` in JS). Called inside an update or read, so
/// it can read nodes.
pub trait SelectionMovement {
    /// The point one `granularity` unit before (`is_backward`) or after
    /// `from`, or `None` when it can't move.
    fn landed_point(
        &self,
        from: &Point,
        is_backward: bool,
        granularity: SelectionGranularity,
    ) -> Result<Option<Point>, LexicalError>;
}

/// `deleteCharacter`'s scan for a block to merge with.
enum MergeState {
    /// `{type: 'initial'}`.
    Initial,
    /// `{type: 'merge-next-block', block}`.
    MergeNextBlock(NodeRef),
    /// `{type: 'merge-block', caret, block}`.
    MergeBlock { caret: NodeCaret, block: NodeRef },
}

fn direction(is_backward: bool) -> CaretDirection {
    if is_backward {
        CaretDirection::Previous
    } else {
        CaretDirection::Next
    }
}

impl SelectionRef {
    /// `selection.deleteCharacter(isBackward)`: deletes one character, or
    /// the selected content, and handles the boundaries of blocks.
    pub fn delete_character(&self, is_backward: bool) -> Result<(), LexicalError> {
        let Some(range) = self.range() else {
            return Ok(());
        };
        let was_collapsed = range.is_collapsed();
        if was_collapsed {
            let anchor = range.anchor.clone();
            let anchor_node = anchor.get_node()?;
            if forward_deletion(&anchor, &anchor_node, is_backward) {
                // Forward deletion stops at an adjacent shadow root, unless
                // the anchor block is empty and the shadow root is its next
                // sibling: the caret walk below then removes the empty block
                // (#8075).
                let next_sibling = if anchor_node.is_element() {
                    anchor_node.get_next_sibling()
                } else {
                    None
                };
                if !(anchor_node.is_element()
                    && anchor_node.is_empty()
                    && next_sibling.is_some_and(|n| is_shadow_root_node(&n)))
                {
                    return Ok(());
                }
            }
            let direction = direction(is_backward);
            let initial_caret = caret_from_point(&anchor, direction)?;
            let initial_range = extend_caret_to_range(&initial_caret)?;
            if initial_range
                .get_text_slices()
                .iter()
                .all(|slice| slice.as_ref().is_none_or(|s| s.distance == 0))
            {
                // The caret is an element point beside a node that renders a
                // block cursor: the keystroke deletes that node.
                if anchor.point_type == PointType::Element
                    && let Some(adjacent) = initial_caret.get_node_at_caret()
                    && adjacent.is_element()
                    && needs_block_cursor_beside(&adjacent)
                {
                    let container = adjacent.get_parent();
                    adjacent.remove(false)?;
                    if let Some(restored) =
                        restore_empty_container_paragraph(container.as_ref(), Some(&adjacent))?
                    {
                        restored.select_start()?;
                    }
                    return Ok(());
                }
                // There's no text in the direction of the deletion so we can
                // explore our options.
                let anchor_origin = initial_range.anchor.origin().clone();
                let mut state = MergeState::Initial;
                for caret in initial_range.iter_node_carets(RootMode::ShadowRoot) {
                    let origin = caret.origin().clone();
                    if caret.is_child_caret() {
                        if origin.is_inline() {
                            // fall through when descending an inline
                        } else if origin.get_latest().is_shadow_root() {
                            if matches!(state, MergeState::MergeBlock { .. }) {
                                break;
                            }
                            // Don't merge with a shadow root block.
                            if anchor_origin.is_element() && anchor_origin.is_empty() {
                                // delete an empty paragraph like the
                                // DecoratorNode case
                                let norm_caret = normalize_caret(&caret);
                                update_range_selection_from_caret_range(
                                    self,
                                    &get_caret_range(norm_caret.clone(), norm_caret)?,
                                )?;
                                anchor_origin.remove(false)?;
                            }
                            return Ok(());
                        } else if let MergeState::MergeNextBlock(block)
                        | MergeState::MergeBlock { block, .. } = &state
                        {
                            // Keep descending ChildCaret to find which block
                            // to merge with.
                            state = MergeState::MergeBlock {
                                block: block.clone(),
                                caret: caret.clone(),
                            };
                        }
                    } else if matches!(state, MergeState::MergeBlock { .. }) {
                        break;
                    } else {
                        if origin.is_element() {
                            if !origin.is_inline() {
                                state = MergeState::MergeNextBlock(origin);
                            } else if !is_parent_of(&origin, &anchor_origin) {
                                break;
                            }
                            continue;
                        }
                        if origin.is_decorator() {
                            if origin.is_isolated() {
                                // do nothing, shouldn't delete an isolated
                                // decorator
                            } else if matches!(state, MergeState::MergeNextBlock(_))
                                && (origin.is_keyboard_selectable() || !origin.is_inline())
                                && anchor_origin.is_element()
                                && anchor_origin.is_empty()
                            {
                                // If the anchor is an empty element that is
                                // adjacent to a decorator then we remove the
                                // paragraph and select the decorator
                                anchor_origin.remove(false)?;
                                let node_selection = create_node_selection();
                                node_selection.add(origin.get_key());
                                set_selection(Some(node_selection));
                            } else {
                                // When the anchor is not an empty element
                                // then the adjacent decorator is removed
                                let container = origin.get_parent();
                                origin.remove(false)?;
                                if let Some(restored) = restore_empty_container_paragraph(
                                    container.as_ref(),
                                    Some(&origin),
                                )? {
                                    restored.select_start()?;
                                }
                            }
                            // always stop when a decorator is encountered
                            return Ok(());
                        }
                        if origin.get_latest().is::<LineBreakNode>() {
                            // A LineBreakNode is a single deletable unit.
                            origin.remove(false)?;
                            return Ok(());
                        }
                        break;
                    }
                }
                if let MergeState::MergeBlock { caret, block } = state {
                    // `$getSlotNames(block)` is empty: slots are not ported.
                    let origin = caret.origin();
                    // Empty adjacent block at the same nesting level: remove
                    // it instead of merging, so the current block's type
                    // survives.
                    if origin.is_empty()
                        && !block.is_empty()
                        && origin.get_parent() == block.get_parent()
                    {
                        origin.remove(true)?;
                        return Ok(());
                    }
                    let range_anchor = if !origin.is_empty() && block.is_empty() {
                        rewind_sibling_caret(&get_sibling_caret(&block, caret.direction()))?
                    } else {
                        initial_range.anchor.clone()
                    };
                    update_range_selection_from_caret_range(
                        self,
                        &get_caret_range(range_anchor, caret)?,
                    )?;
                    return self.remove_text();
                }
                // JS stops here when the caret sits at the edge of a slot
                // value; slots are not ported.
            }

            // Handle the deletion around decorators.
            extend_selection_for_deletion(self, is_backward, SelectionGranularity::Character)?;

            let range = self.range().expect("a range selection");
            if !range.is_collapsed() {
                let focus = &range.focus;
                let anchor = &range.anchor;
                let focus_node = match focus.point_type {
                    PointType::Text => Some(focus.get_node()?),
                    PointType::Element => None,
                };
                let anchor_node = match anchor.point_type {
                    PointType::Text => Some(anchor.get_node()?),
                    PointType::Element => None,
                };
                if let Some(focus_node) = focus_node.as_ref().filter(|n| n.is_segmented()) {
                    let offset = focus.offset;
                    let text_content_size = focus_node.get_text_content_size();
                    if anchor_node.as_ref() == Some(focus_node)
                        || (is_backward && offset != text_content_size)
                        || (!is_backward && offset != 0)
                    {
                        return remove_segment(focus_node, is_backward, offset);
                    }
                } else if let Some(anchor_node) = anchor_node.as_ref().filter(|n| n.is_segmented())
                {
                    let offset = anchor.offset;
                    let text_content_size = anchor_node.get_text_content_size();
                    if focus_node.as_ref() == Some(anchor_node)
                        || (is_backward && offset != 0)
                        || (!is_backward && offset != text_content_size)
                    {
                        return remove_segment(anchor_node, is_backward, offset);
                    }
                }
                update_caret_selection_for_unicode_character(self, is_backward)?;
            } else if is_backward
                && range.anchor.offset == 0
                // Special handling around rich text nodes
                && collapse_at_start(self, &range.anchor.get_node()?)?
            {
                return Ok(());
            }
        }
        if !was_collapsed {
            // Widen a whole-document range to the blocks themselves (#5835).
            internal_expand_selection_to_whole_document(self)?;
        }
        self.remove_text()?;
        if is_backward
            && !was_collapsed
            && self.range().is_some_and(|r| {
                r.is_collapsed()
                    && r.anchor.point_type == PointType::Element
                    && r.anchor.offset == 0
            })
        {
            ensure_root_has_paragraph()?;
        }
        Ok(())
    }

    /// `selection.deleteWord(isBackward)`.
    pub fn delete_word(&self, is_backward: bool) -> Result<(), LexicalError> {
        delete_text_by_granularity(self, is_backward, SelectionGranularity::Word)
    }

    /// `selection.deleteLine(isBackward)`.
    pub fn delete_line(&self, is_backward: bool) -> Result<(), LexicalError> {
        delete_text_by_granularity(self, is_backward, SelectionGranularity::LineBoundary)
    }

    /// `NodeSelection.deleteNodes()`: removes the selected nodes, first
    /// moving the selection to just after the first of them when this is
    /// the current (or last committed) selection, and gives an emptied root
    /// a paragraph. (JS skips slotted nodes; slots are not ported.)
    pub fn delete_nodes(&self) -> Result<(), LexicalError> {
        let nodes = self.get_nodes()?;
        let current = get_selection().or_else(get_previous_selection);
        if let (Some(current), Some(first)) = (current, nodes.first())
            && current.ptr_eq(self)
        {
            let first_caret = get_sibling_caret(first, CaretDirection::Next);
            set_selection_from_caret_range(&get_caret_range(first_caret.clone(), first_caret)?)?;
        }
        for node in &nodes {
            node.remove(false)?;
        }
        ensure_root_has_paragraph()
    }
}

/// `selection.forwardDeletion(anchor, anchorNode, isBackward)`: whether a
/// forward deletion at the end of the anchor node would reach a shadow
/// root, which it must not merge into.
fn forward_deletion(anchor: &Point, anchor_node: &NodeRef, is_backward: bool) -> bool {
    let at_end = match anchor.point_type {
        PointType::Element => {
            anchor_node.is_element() && anchor.offset == anchor_node.get_children_size()
        }
        PointType::Text => anchor.offset == anchor_node.get_text_content_size(),
    };
    if !is_backward && at_end {
        let next_sibling = anchor_node
            .get_next_sibling()
            .or_else(|| anchor_node.get_parent().and_then(|p| p.get_next_sibling()));
        if next_sibling.is_some_and(|n| is_shadow_root_node(&n)) {
            return true;
        }
    }
    false
}

/// `$deleteTextByGranularity(selection, isBackward, granularity)`: word and
/// line deletion.
fn delete_text_by_granularity(
    selection: &SelectionRef,
    is_backward: bool,
    granularity: SelectionGranularity,
) -> Result<(), LexicalError> {
    let Some(range) = selection.range() else {
        return Ok(());
    };
    let was_collapsed = range.is_collapsed();
    // Set when the deletion stops at a slot boundary, which it never crosses.
    let mut stopped_at_slot = false;
    if was_collapsed {
        if granularity == SelectionGranularity::Word
            && forward_deletion(&range.anchor, &range.anchor.get_node()?, is_backward)
        {
            return Ok(());
        }
        stopped_at_slot = extend_selection_for_deletion(selection, is_backward, granularity)?;
        if granularity == SelectionGranularity::LineBoundary {
            stop_line_deletion_at_line_break(selection)?;
        }
    }
    // Line deletion must remain in one block; word deletion may cross blocks.
    let range = selection.range().expect("a range selection");
    if granularity == SelectionGranularity::LineBoundary
        && !range.is_collapsed()
        && find_block(&range.anchor.get_node()?) != find_block(&range.focus.get_node()?)
    {
        let anchor = &range.anchor;
        selection.point_set(Which::Focus, &anchor.key, anchor.offset, anchor.point_type)?;
    }
    if selection.range().is_some_and(|r| r.is_collapsed()) {
        // Stopped at a slot boundary with nothing before it: a no-op.
        if !stopped_at_slot {
            selection.delete_character(is_backward)?;
        }
    } else {
        // An existing whole-document selection deletes its blocks (#5835).
        if !was_collapsed {
            internal_expand_selection_to_whole_document(selection)?;
        }
        selection.remove_text()?;
    }
    Ok(())
}

/// `$findMatchingParent(node, INTERNAL_$isBlock)`: the node or its nearest
/// ancestor that is a block.
fn find_block(node: &NodeRef) -> Option<NodeRef> {
    let mut current = Some(node.clone());
    while let Some(node) = current {
        if is_block(&node) {
            return Some(node);
        }
        current = node.get_parent();
    }
    None
}

/// `$stopLineDeletionAtLineBreak(selection)`: pulls a line deletion's focus
/// back to the anchor's side of the first line break between them.
fn stop_line_deletion_at_line_break(selection: &SelectionRef) -> Result<(), LexicalError> {
    let range = selection.range().expect("a range selection");
    for caret in caret_range_from_selection(&range)?.iter_node_carets(RootMode::ShadowRoot) {
        if caret.is_sibling_caret() && caret.origin().get_latest().is::<LineBreakNode>() {
            return set_point_from_caret(selection, Which::Focus, &rewind_sibling_caret(&caret)?);
        }
    }
    Ok(())
}

/// The editor's [`SelectionMovement`], what JS reaches through
/// `getDOMSelection(getWindow(editor))`: without one, `getWindow` throws as
/// it does for an editor with no root element.
fn selection_movement() -> Result<Rc<dyn SelectionMovement>, LexicalError> {
    let editor =
        get_active_editor().ok_or_else(|| LexicalError::new("Unable to find an active editor."))?;
    editor
        .selection_movement()
        .ok_or_else(|| LexicalError::new("window object not found"))
}

/// Where the native `Selection.modify` leaves the caret it moves from
/// `from`: the movement's landed point, or `from` when it can't move.
fn landed_point(
    movement: &dyn SelectionMovement,
    from: &Point,
    is_backward: bool,
    granularity: SelectionGranularity,
) -> Result<Point, LexicalError> {
    Ok(movement
        .landed_point(from, is_backward, granularity)?
        .unwrap_or_else(|| from.clone()))
}

impl SelectionRef {
    /// `selection.modify(alter, isBackward, granularity)`: moves (collapsed)
    /// or extends the selection by one unit, across decorators and blocks
    /// in the model and otherwise where the editor's [`SelectionMovement`]
    /// lands. Without a movement this fails with `window object not found`
    /// unless the model step handled it, as JS does with no root element.
    pub fn modify(
        &self,
        alter: SelectionAlter,
        is_backward: bool,
        granularity: SelectionGranularity,
    ) -> Result<(), LexicalError> {
        let Some(range) = self.range() else {
            return Ok(());
        };
        if modify_selection_around_decorators_and_blocks(
            self,
            alter,
            is_backward,
            granularity,
            false,
        )? {
            return Ok(());
        }
        let collapse = alter == SelectionAlter::Move;
        let movement = selection_movement()?;
        // Removing the DOM block cursor element: there is no DOM. When the
        // selection is dirty JS first syncs the DOM selection to it; the
        // movement always measures from the model's focus.
        let focus = range.focus.clone();
        let focus_node = focus.get_node()?;
        let mut from = focus.clone();
        // When focus sits at a TextNode boundary, pre-normalize the caret
        // into the adjacent sibling's text so the movement can cross
        // inline-grid/flex spans (#7301).
        if granularity == SelectionGranularity::Character
            && focus_node.is_text()
            && focus_node.is_unmergeable()
        {
            let at_boundary = if is_backward {
                focus.offset == 0
            } else {
                focus.offset == focus_node.get_text_content_size()
            };
            if at_boundary
                && let Some(sibling) = get_sibling_caret(&focus_node, direction(is_backward))
                    .get_node_at_caret()
                    .filter(NodeRef::is_text)
            {
                let sibling_size = sibling.get_text_content_size();
                if collapse {
                    let offset = if is_backward { sibling_size } else { 0 };
                    from = Point::new(sibling.get_key(), offset, PointType::Text);
                } else {
                    // For extend (used by deleteCharacter), set the selection
                    // directly instead of measuring.
                    let offset = if is_backward {
                        sibling_size.saturating_sub(1)
                    } else {
                        1
                    };
                    self.point_set(Which::Focus, sibling.get_key(), offset, PointType::Text)?;
                    self.set_dirty(true);
                    return Ok(());
                }
            }
        }
        let landed = landed_point(&*movement, &from, is_backward, granularity)?;
        // The DOM selection after the native move: collapsed at the landed
        // point, or from the anchor to it. Its range is in document order.
        let (dom_anchor, dom_focus) = if collapse {
            (landed.clone(), landed)
        } else {
            (range.anchor.clone(), landed)
        };
        let dom_is_backward = dom_focus.is_before(&dom_anchor)?;
        let (start, end) = if dom_is_backward {
            (dom_focus, dom_anchor)
        } else {
            (dom_anchor, dom_focus)
        };
        // Apply the DOM selection to our Lexical selection.
        let anchor_node = self.range().expect("a range selection").anchor.get_node()?;
        let root = if anchor_node.is_root() {
            anchor_node
        } else {
            get_nearest_root_or_shadow_root(&anchor_node)?
        };
        apply_range(self, &start, &end)?;
        self.set_dirty(true);
        if !collapse {
            shrink_selection_to_root(self, is_backward, &root)?;
            // Because a range works on start and end, we might need to flip
            // the anchor and focus points to match what the DOM has.
            if dom_is_backward {
                swap_points(self)?;
            }
        }
        if granularity == SelectionGranularity::LineBoundary {
            modify_selection_around_decorators_and_blocks(
                self,
                alter,
                is_backward,
                granularity,
                true,
            )?;
        }
        Ok(())
    }
}

/// `$extendSelectionForDeletion(selection, isBackward, granularity)`:
/// extends the collapsed selection over what the deletion removes. Returns
/// true when the focus stopped at a slot boundary (never, as slots are not
/// ported).
///
/// The moves across blocks are resolved in the model; every other move is
/// measured by the editor's [`SelectionMovement`] where JS uses the DOM
/// selection of `getWindow(editor)`, and fails like JS without one.
fn extend_selection_for_deletion(
    selection: &SelectionRef,
    is_backward: bool,
    granularity: SelectionGranularity,
) -> Result<bool, LexicalError> {
    // Decorator/block handling, resolved in the model exactly as modify()
    // does.
    if modify_selection_around_decorators_and_blocks(
        selection,
        SelectionAlter::Extend,
        is_backward,
        granularity,
        false,
    )? {
        return Ok(false);
    }
    let movement = selection_movement()?;
    // Removing the DOM block cursor element: there is no DOM.
    let range = selection.range().expect("a range selection");
    let anchor = range.anchor.clone();
    let anchor_node = anchor.get_node()?;
    // Measure from the focus, which the pre-pass above may have hopped past
    // an inline decorator; for an untouched collapsed selection it is the
    // anchor.
    let was_collapsed = range.is_collapsed();
    let landed = landed_point(&*movement, &range.focus, is_backward, granularity)?;
    // JS resolves a line boundary that lands in an inline decorator's
    // private DOM text at the decorator's edge; a movement lands on model
    // points, never in a decorator's DOM.

    // The movement may not cross inline-grid/flex span boundaries (#7301).
    // When at the deletion-side edge of an unmergeable TextNode, extend into
    // the adjacent sibling directly instead of relying on the movement.
    if was_collapsed
        && granularity == SelectionGranularity::Character
        && anchor.point_type == PointType::Text
        && anchor_node.is_unmergeable()
    {
        let boundary_offset = if is_backward {
            0
        } else {
            anchor_node.get_text_content_size()
        };
        if anchor.offset == boundary_offset
            && let Some(sibling) = get_sibling_caret(&anchor_node, direction(is_backward))
                .get_node_at_caret()
                .filter(NodeRef::is_text)
        {
            let sibling_offset = if is_backward {
                sibling.get_text_content_size().saturating_sub(1)
            } else {
                1
            };
            selection.point_set(
                Which::Focus,
                sibling.get_key(),
                sibling_offset,
                PointType::Text,
            )?;
            selection.set_dirty(true);
            return Ok(false);
        }
    }
    // In-node character deletion (the common case): keep the focus in the
    // anchor's own text node. Landing inside the anchor node uses that
    // offset; landing anywhere else is clamped to the anchor's edge.
    if was_collapsed
        && granularity == SelectionGranularity::Character
        && anchor.point_type == PointType::Text
    {
        // The deletion-side edge of the anchor's text.
        let edge_offset = if is_backward {
            0
        } else {
            anchor_node.get_text_content_size()
        };
        let clamped_offset = if landed.point_type == PointType::Text && landed.key == anchor.key {
            Some(landed.offset)
        } else if anchor.offset != edge_offset {
            Some(edge_offset)
        } else {
            None
        };
        if let Some(clamped_offset) = clamped_offset {
            if clamped_offset != anchor.offset {
                selection.point_set(Which::Focus, &anchor.key, clamped_offset, PointType::Text)?;
                selection.set_dirty(true);
            }
            return Ok(false);
        }
    }
    // JS stops the focus at a slot boundary here; slots are not ported.

    // General path: the [original .. landed] range a native extend would
    // have produced, in document order.
    let (start, end) = if is_backward {
        (landed, anchor)
    } else {
        (anchor, landed)
    };
    let root = if anchor_node.is_root() {
        anchor_node
    } else {
        get_nearest_root_or_shadow_root(&anchor_node)?
    };
    apply_range(selection, &start, &end)?;
    selection.set_dirty(true);
    if !shrink_selection_to_root(selection, is_backward, &root)? && is_backward {
        // applyDOMRange set anchor = range start (the landed point); the
        // deletion anchor must stay at the original caret, so restore that
        // orientation.
        swap_points(selection)?;
    }
    Ok(false)
}

/// `selection.applyDOMRange(range)` for a range whose ends are model points
/// already: resolves them as `$internalResolveSelectionPoints` does
/// (`$internalResolveSelectionPoint` and
/// `$normalizeSelectionPointsForBoundaries`), sets them as the anchor and
/// the focus, and normalizes the selection (`$normalizeSelection`).
fn apply_range(selection: &SelectionRef, start: &Point, end: &Point) -> Result<(), LexicalError> {
    let last_selection = get_active_editor()
        .and_then(|editor| editor.get_editor_state().selection())
        .and_then(|s| s.range());
    let mut anchor = resolve_selection_point(start, last_selection.as_ref().map(|r| &r.anchor))?;
    let mut focus = resolve_selection_point(end, last_selection.as_ref().map(|r| &r.focus))?;
    // JS ignores a range whose element points are both inside decorators'
    // DOM; model points are never inside one.
    normalize_selection_points_for_boundaries(&mut anchor, &mut focus)?;
    selection.point_set(Which::Anchor, &anchor.key, anchor.offset, anchor.point_type)?;
    selection.point_set(Which::Focus, &focus.key, focus.offset, focus.point_type)?;
    // Firefox will use an element point rather than a text point in some
    // cases, so we normalize for that
    normalize_selection(selection)
}

/// `$internalResolveSelectionPoint(dom, offset, lastPoint)` for the DOM
/// position of a model point: a text point keeps its node (its offset
/// clamped), and an element point resolves to the text at that child, as the
/// DOM resolution of an element's child offset does (an element's DOM
/// children are its children here, with no block cursor).
fn resolve_selection_point(
    point: &Point,
    last_point: Option<&Point>,
) -> Result<Point, LexicalError> {
    let node = point.get_node()?;
    if point.point_type == PointType::Text {
        let offset = point.offset.min(node.get_text_content_size());
        return Ok(Point::new(point.key.clone(), offset, PointType::Text));
    }
    let size = node.get_children_size();
    let mut resolved_offset = point.offset;
    // If the anchor is the same as length, then this means we need to select
    // the very last text node.
    let move_selection_to_end = resolved_offset == size && size > 0;
    if move_selection_to_end {
        resolved_offset = size - 1;
    }
    let text_point = |text: &NodeRef| {
        let offset = if move_selection_to_end {
            text.get_text_content_size()
        } else {
            0
        };
        Point::new(text.get_key(), offset, PointType::Text)
    };
    if let Some(text) = node
        .get_child_at_index(resolved_offset)
        .filter(NodeRef::is_text)
    {
        return Ok(text_point(&text));
    }
    let mut resolved_element = node;
    let mut child = resolved_element.get_child_at_index(resolved_offset);
    let element_child = child.clone().filter(|c| c.is_element());
    if let Some(element) = element_child
        && should_resolve_ancestor(&element, last_point)?
    {
        let descendant = if move_selection_to_end {
            element.get_last_descendant()
        } else {
            element.get_first_descendant()
        };
        match descendant {
            None => resolved_element = element,
            Some(descendant) => {
                resolved_element = if descendant.is_element() {
                    descendant.clone()
                } else {
                    descendant.get_parent_or_throw()?
                };
                child = Some(descendant);
            }
        }
        resolved_offset = 0;
    }
    if let Some(text) = child.as_ref().filter(|c| c.is_text()) {
        return Ok(text_point(text));
    }
    if child.as_ref() != Some(&resolved_element) && move_selection_to_end {
        resolved_offset = (resolved_offset + 1).min(resolved_element.get_children_size());
    }
    Ok(Point::new(
        resolved_element.get_key(),
        resolved_offset,
        PointType::Element,
    ))
}

/// `shouldResolveAncestor(resolvedElement, resolvedOffset, lastPoint)`.
fn should_resolve_ancestor(
    resolved_element: &NodeRef,
    last_point: Option<&Point>,
) -> Result<bool, LexicalError> {
    let Some(last_point) = last_point else {
        return Ok(true);
    };
    Ok(match resolved_element.get_parent() {
        None => true,
        Some(parent) => !parent.can_be_empty() || parent != last_point.get_node()?,
    })
}

/// `$normalizeSelectionPointsForBoundaries(anchor, focus, lastSelection)`.
fn normalize_selection_points_for_boundaries(
    anchor: &mut Point,
    focus: &mut Point,
) -> Result<(), LexicalError> {
    if anchor.point_type == PointType::Text && focus.point_type == PointType::Text {
        let is_backward = anchor.is_before(focus)?;
        let is_collapsed = anchor == focus;
        // Attempt to normalize the offset to the previous sibling if we're
        // at the start of a text node and the sibling is a text node or
        // inline element.
        resolve_selection_point_on_boundary(anchor, is_backward, is_collapsed)?;
        resolve_selection_point_on_boundary(focus, !is_backward, is_collapsed)?;
        if is_collapsed {
            *focus = anchor.clone();
        }
    }
    Ok(())
}

/// `$resolveSelectionPointOnBoundary(point, isBackward, isCollapsed)` (a
/// headless editor never has an empty composition target).
fn resolve_selection_point_on_boundary(
    point: &mut Point,
    is_backward: bool,
    is_collapsed: bool,
) -> Result<(), LexicalError> {
    let node = point.get_node()?;
    let before = point.offset == 0;
    if !before && point.offset != node.get_text_content_size() {
        return Ok(());
    }
    let direction = if before {
        CaretDirection::Previous
    } else {
        CaretDirection::Next
    };
    let sibling = get_sibling_caret(&node, direction).get_node_at_caret();
    if before != is_backward
        && let Some(sibling) = sibling.as_ref().filter(|s| s.is_element() && s.is_inline())
        && (!before || !is_collapsed)
    {
        let offset = if before {
            sibling.get_children_size()
        } else {
            0
        };
        *point = Point::new(sibling.get_key(), offset, PointType::Element);
    } else if before && !is_backward {
        if let Some(sibling) = sibling.filter(NodeRef::is_text)
            && !node.is_unmergeable()
        {
            *point = Point::new(
                sibling.get_key(),
                sibling.get_text_content_size(),
                PointType::Text,
            );
        }
    } else if sibling.is_none()
        && (is_collapsed || (!before && is_backward))
        && let Some(parent) = node.get_parent()
        && parent.is_element()
        && parent.is_inline()
        && (before
            || (!parent.get_latest().can_insert_text_after() && parent.get_text_content_size() > 1))
        && let Some(parent_sibling) = get_sibling_caret(&parent, direction)
            .get_node_at_caret()
            .filter(NodeRef::is_text)
    {
        let offset = if before {
            parent_sibling.get_text_content_size()
        } else {
            0
        };
        *point = Point::new(parent_sibling.get_key(), offset, PointType::Text);
    }
    Ok(())
}

/// `$shrinkSelectionToRoot(selection, isBackward, root)`: when part of the
/// selection lies outside `root`, shrinks it to the edge of the part inside.
/// Returns true if the selection was shrunk.
fn shrink_selection_to_root(
    selection: &SelectionRef,
    is_backward: bool,
    root: &NodeRef,
) -> Result<bool, LexicalError> {
    let nodes = selection.get_nodes()?;
    let valid_nodes: Vec<&NodeRef> = nodes.iter().filter(|n| is_parent_of(root, n)).collect();
    if valid_nodes.is_empty() || valid_nodes.len() == nodes.len() {
        return Ok(false);
    }
    let edge_node = if is_backward {
        valid_nodes[0]
    } else {
        valid_nodes[valid_nodes.len() - 1]
    };
    let edge_element = if edge_node.is_element() {
        edge_node.clone()
    } else {
        edge_node.get_parent_or_throw()?
    };
    if is_backward {
        edge_element.select_start()?;
    } else {
        edge_element.select_end()?;
    }
    Ok(true)
}

/// `$swapPoints(selection)`.
fn swap_points(selection: &SelectionRef) -> Result<(), LexicalError> {
    let range = selection.range().expect("a range selection");
    let (anchor, focus) = (range.anchor, range.focus);
    selection.point_set(Which::Anchor, &focus.key, focus.offset, focus.point_type)?;
    selection.point_set(Which::Focus, &anchor.key, anchor.offset, anchor.point_type)
}

/// `$getNearestRootOrShadowRoot(node)` (slots are not ported).
fn get_nearest_root_or_shadow_root(node: &NodeRef) -> Result<NodeRef, LexicalError> {
    let mut current = node.clone();
    loop {
        let parent = current.get_parent_or_throw()?;
        if is_root_or_shadow_root(&*parent.get_latest()) {
            return Ok(parent);
        }
        current = parent;
    }
}

/// `$modifySelectionAroundDecoratorsAndBlocks(selection, alter, isBackward,
/// granularity, mode)`: resolves in the model a move or extension over a
/// decorator or out of the caret's block (`decorators_only` is the
/// `'decorators'` mode, which doesn't look for blocks), and returns true
/// when that handled it; false when the native measurement has to resolve
/// the move.
fn modify_selection_around_decorators_and_blocks(
    selection: &SelectionRef,
    alter: SelectionAlter,
    is_backward: bool,
    granularity: SelectionGranularity,
    decorators_only: bool,
) -> Result<bool, LexicalError> {
    let range = selection.range().expect("a range selection");
    if alter == SelectionAlter::Move
        && granularity == SelectionGranularity::Character
        && !range.is_collapsed()
    {
        // moving left or right when the selection isn't collapsed will just
        // set the anchor to the focus or vice versa depending on direction
        let (src, dst) = if is_backward == selection.is_backward()? {
            (&range.focus, Which::Anchor)
        } else {
            (&range.anchor, Which::Focus)
        };
        selection.point_set(dst, &src.key, src.offset, src.point_type)?;
        return Ok(true);
    }
    let initial_focus = caret_from_point(&range.focus, direction(is_backward))?;
    let is_line_boundary = granularity == SelectionGranularity::LineBoundary;
    let collapse = alter == SelectionAlter::Move;
    // `focus === initialFocus` in JS: `None` until another caret is chosen.
    let mut focus: Option<PointCaret> = None;
    let mut check_for_block = !decorators_only;
    // An isolated decorator stops the caret.
    let mut isolated = false;
    if !is_extendable_text_point_caret(&initial_focus) {
        for sibling_caret in initial_focus.iter() {
            check_for_block = false;
            let origin = sibling_caret.origin().clone();
            if origin.is_decorator() {
                if origin.is_isolated() {
                    isolated = true;
                    break;
                }
                focus = Some(sibling_caret);
                if is_line_boundary && origin.is_inline() {
                    continue;
                }
            }
            break;
        }
        if isolated {
            // Handled, with the selection deliberately left where it is.
            return Ok(true);
        }
        if check_for_block {
            let root_mode = match alter {
                SelectionAlter::Extend => RootMode::ShadowRoot,
                SelectionAlter::Move => RootMode::Root,
            };
            for next_caret in extend_caret_to_range(&initial_focus)?.iter_node_carets(root_mode) {
                if next_caret.is_child_caret() {
                    if !next_caret.origin().is_inline() {
                        focus = Some(next_caret);
                    }
                } else if next_caret.origin().is_element() {
                    continue;
                } else if next_caret.origin().is_decorator() && !next_caret.origin().is_inline() {
                    focus = Some(next_caret);
                }
                break;
            }
        }
    }
    let Some(focus) = focus else {
        return Ok(false);
    };
    // After this point checkForBlock is true if and only if we moved to a
    // different block, so we should stop regardless of the granularity
    if collapse
        && !is_line_boundary
        && focus.origin().is_decorator()
        && focus.origin().is_keyboard_selectable()
    {
        // Make it possible to move selection from range selection to node
        // selection on the node.
        let node_selection = create_node_selection();
        node_selection.add(focus.origin().get_key());
        set_selection(Some(node_selection));
        return Ok(true);
    }
    let focus = normalize_caret(&focus);
    if collapse {
        set_point_from_caret(selection, Which::Anchor, &focus)?;
    }
    set_point_from_caret(selection, Which::Focus, &focus)?;
    Ok(check_for_block || !is_line_boundary)
}

/// `$needsBlockCursorBeside(node)`: whether a caret beside the node needs a
/// block cursor, as a block decorator, a top-level shadow root and an
/// element that can't be empty do.
fn needs_block_cursor_beside(node: &NodeRef) -> bool {
    if node.is_inline() {
        return false;
    }
    if node.is_decorator() {
        return true;
    }
    if node.is_element() {
        if node.get_latest().is_shadow_root() {
            return !node.get_parent().is_some_and(|p| is_shadow_root_node(&p));
        }
        return !node.can_be_empty();
    }
    false
}

/// `node.isParentOf(targetNode)` (`$hasAncestor(targetNode, node)`).
fn is_parent_of(node: &NodeRef, target: &NodeRef) -> bool {
    let mut parent = target.get_parent();
    while let Some(p) = parent {
        if &p == node {
            return true;
        }
        parent = p.get_parent();
    }
    false
}

/// `$collapseAtStart(selection, startNode)`: lets the elements the
/// selection is at the start of handle a backward deletion there.
fn collapse_at_start(selection: &SelectionRef, start_node: &NodeRef) -> Result<bool, LexicalError> {
    let mut node = Some(start_node.clone());
    while let Some(n) = node {
        if n.is_element() {
            if n.collapse_at_start(selection)? {
                return Ok(true);
            }
            if is_root_or_shadow_root(&*n.get_latest()) {
                break;
            }
        }
        if n.get_previous_sibling().is_some() {
            break;
        }
        node = n.get_parent();
    }
    Ok(false)
}

/// `INTERNAL_$expandSelectionToWholeDocument(selection)`: widens a
/// selection that covers the whole document to the root's element points,
/// so a deletion removes the blocks rather than emptying them.
fn internal_expand_selection_to_whole_document(
    selection: &SelectionRef,
) -> Result<(), LexicalError> {
    let root = get_root();
    let range = selection.range().expect("a range selection");
    if root.is_empty() || !is_block_fully_selected(&root, &range)? {
        return Ok(());
    }
    selection.point_set(Which::Anchor, root.get_key(), 0, PointType::Element)?;
    selection.point_set(
        Which::Focus,
        root.get_key(),
        root.get_children_size(),
        PointType::Element,
    )
}

/// `$isBlockFullySelected(blockNode, selection)`: whether the selection
/// covers everything in the block. (Slot frames are not ported, so every
/// node is in the same frame.)
fn is_block_fully_selected(
    block: &NodeRef,
    selection: &RangeSelection,
) -> Result<bool, LexicalError> {
    let range = get_caret_range_in_direction(
        &caret_range_from_selection(selection)?,
        CaretDirection::Next,
    )?;
    let block_start = normalize_caret(
        &get_child_caret(block, CaretDirection::Next).expect("a block is an element"),
    );
    let block_end = get_caret_in_direction(
        &normalize_caret(
            &get_child_caret(block, CaretDirection::Previous).expect("a block is an element"),
        ),
        CaretDirection::Next,
    )?;
    Ok(
        compare_point_caret_next(&range.anchor, &block_start)?.is_le()
            && compare_point_caret_next(&range.focus, &block_end)?.is_ge(),
    )
}

/// `$ensureRootHasParagraph()`: gives an empty root a selected paragraph.
/// (A selection in a slot keeps it from doing so in JS; slots are not
/// ported.)
fn ensure_root_has_paragraph() -> Result<(), LexicalError> {
    let root = get_root();
    if root.get_children_size() == 0 {
        let paragraph = create_paragraph_node();
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.select(None, None)?;
    }
    Ok(())
}

/// `$updateCaretSelectionForUnicodeCharacter(selection, isBackward)`:
/// narrows a deletion within one text node to one code unit, unless the
/// text holds a surrogate pair or an emoji.
fn update_caret_selection_for_unicode_character(
    selection: &SelectionRef,
    is_backward: bool,
) -> Result<(), LexicalError> {
    let range = selection.range().expect("a range selection");
    let anchor = &range.anchor;
    let focus = &range.focus;
    if anchor.key == focus.key
        && anchor.point_type == PointType::Text
        && focus.point_type == PointType::Text
    {
        // Handling of multibyte characters
        let anchor_offset = anchor.offset;
        let focus_offset = focus.offset;
        let is_before = anchor_offset < focus_offset;
        let start_offset = if is_before {
            anchor_offset
        } else {
            focus_offset
        };
        let end_offset = if is_before {
            focus_offset
        } else {
            anchor_offset
        };
        let character_offset = end_offset as isize - 1;
        if start_offset as isize != character_offset {
            let text = utf16::slice(
                &anchor.get_node()?.get_text_content(),
                start_offset,
                end_offset,
            );
            if should_delete_exactly_one_code_unit(&text)? {
                let character_offset = character_offset as usize;
                if is_backward {
                    selection.point_set(
                        Which::Focus,
                        &focus.key,
                        character_offset,
                        focus.point_type,
                    )?;
                } else {
                    selection.point_set(
                        Which::Anchor,
                        &anchor.key,
                        character_offset,
                        anchor.point_type,
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// `shouldDeleteExactlyOneCodeUnit(text)`.
fn should_delete_exactly_one_code_unit(text: &str) -> Result<bool, LexicalError> {
    // JS checks this in DEV builds, which the vectors come from.
    invariant!(
        utf16::len(text) > 1,
        "shouldDeleteExactlyOneCodeUnit: expecting to be called only with sequences of two or more code units"
    );
    Ok(!(does_contain_surrogate_pair(text) || does_contain_emoji(text)))
}

/// `doesContainSurrogatePair(str)`: a character outside the BMP. (A half
/// pair is U+FFFD here, which JS's regex doesn't match either.)
fn does_contain_surrogate_pair(text: &str) -> bool {
    text.chars().any(|c| c.len_utf16() == 2)
}

/// `doesContainEmoji(text)`: `/\p{Emoji}/u.test(text)`.
fn does_contain_emoji(text: &str) -> bool {
    text.chars().any(|c| {
        let c = u32::from(c);
        EMOJI
            .binary_search_by(|&(lo, hi)| {
                if hi < c {
                    std::cmp::Ordering::Less
                } else if lo > c {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    })
}

/// The Unicode `Emoji` property, as ranges of code points, from Node 24
/// (Unicode 17.0) `/\p{Emoji}/u`.
const EMOJI: &[(u32, u32)] = &[
    (0x23, 0x23),
    (0x2A, 0x2A),
    (0x30, 0x39),
    (0xA9, 0xA9),
    (0xAE, 0xAE),
    (0x203C, 0x203C),
    (0x2049, 0x2049),
    (0x2122, 0x2122),
    (0x2139, 0x2139),
    (0x2194, 0x2199),
    (0x21A9, 0x21AA),
    (0x231A, 0x231B),
    (0x2328, 0x2328),
    (0x23CF, 0x23CF),
    (0x23E9, 0x23F3),
    (0x23F8, 0x23FA),
    (0x24C2, 0x24C2),
    (0x25AA, 0x25AB),
    (0x25B6, 0x25B6),
    (0x25C0, 0x25C0),
    (0x25FB, 0x25FE),
    (0x2600, 0x2604),
    (0x260E, 0x260E),
    (0x2611, 0x2611),
    (0x2614, 0x2615),
    (0x2618, 0x2618),
    (0x261D, 0x261D),
    (0x2620, 0x2620),
    (0x2622, 0x2623),
    (0x2626, 0x2626),
    (0x262A, 0x262A),
    (0x262E, 0x262F),
    (0x2638, 0x263A),
    (0x2640, 0x2640),
    (0x2642, 0x2642),
    (0x2648, 0x2653),
    (0x265F, 0x2660),
    (0x2663, 0x2663),
    (0x2665, 0x2666),
    (0x2668, 0x2668),
    (0x267B, 0x267B),
    (0x267E, 0x267F),
    (0x2692, 0x2697),
    (0x2699, 0x2699),
    (0x269B, 0x269C),
    (0x26A0, 0x26A1),
    (0x26A7, 0x26A7),
    (0x26AA, 0x26AB),
    (0x26B0, 0x26B1),
    (0x26BD, 0x26BE),
    (0x26C4, 0x26C5),
    (0x26C8, 0x26C8),
    (0x26CE, 0x26CF),
    (0x26D1, 0x26D1),
    (0x26D3, 0x26D4),
    (0x26E9, 0x26EA),
    (0x26F0, 0x26F5),
    (0x26F7, 0x26FA),
    (0x26FD, 0x26FD),
    (0x2702, 0x2702),
    (0x2705, 0x2705),
    (0x2708, 0x270D),
    (0x270F, 0x270F),
    (0x2712, 0x2712),
    (0x2714, 0x2714),
    (0x2716, 0x2716),
    (0x271D, 0x271D),
    (0x2721, 0x2721),
    (0x2728, 0x2728),
    (0x2733, 0x2734),
    (0x2744, 0x2744),
    (0x2747, 0x2747),
    (0x274C, 0x274C),
    (0x274E, 0x274E),
    (0x2753, 0x2755),
    (0x2757, 0x2757),
    (0x2763, 0x2764),
    (0x2795, 0x2797),
    (0x27A1, 0x27A1),
    (0x27B0, 0x27B0),
    (0x27BF, 0x27BF),
    (0x2934, 0x2935),
    (0x2B05, 0x2B07),
    (0x2B1B, 0x2B1C),
    (0x2B50, 0x2B50),
    (0x2B55, 0x2B55),
    (0x3030, 0x3030),
    (0x303D, 0x303D),
    (0x3297, 0x3297),
    (0x3299, 0x3299),
    (0x1F004, 0x1F004),
    (0x1F0CF, 0x1F0CF),
    (0x1F170, 0x1F171),
    (0x1F17E, 0x1F17F),
    (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A),
    (0x1F1E6, 0x1F1FF),
    (0x1F201, 0x1F202),
    (0x1F21A, 0x1F21A),
    (0x1F22F, 0x1F22F),
    (0x1F232, 0x1F23A),
    (0x1F250, 0x1F251),
    (0x1F300, 0x1F321),
    (0x1F324, 0x1F393),
    (0x1F396, 0x1F397),
    (0x1F399, 0x1F39B),
    (0x1F39E, 0x1F3F0),
    (0x1F3F3, 0x1F3F5),
    (0x1F3F7, 0x1F4FD),
    (0x1F4FF, 0x1F53D),
    (0x1F549, 0x1F54E),
    (0x1F550, 0x1F567),
    (0x1F56F, 0x1F570),
    (0x1F573, 0x1F57A),
    (0x1F587, 0x1F587),
    (0x1F58A, 0x1F58D),
    (0x1F590, 0x1F590),
    (0x1F595, 0x1F596),
    (0x1F5A4, 0x1F5A5),
    (0x1F5A8, 0x1F5A8),
    (0x1F5B1, 0x1F5B2),
    (0x1F5BC, 0x1F5BC),
    (0x1F5C2, 0x1F5C4),
    (0x1F5D1, 0x1F5D3),
    (0x1F5DC, 0x1F5DE),
    (0x1F5E1, 0x1F5E1),
    (0x1F5E3, 0x1F5E3),
    (0x1F5E8, 0x1F5E8),
    (0x1F5EF, 0x1F5EF),
    (0x1F5F3, 0x1F5F3),
    (0x1F5FA, 0x1F64F),
    (0x1F680, 0x1F6C5),
    (0x1F6CB, 0x1F6D2),
    (0x1F6D5, 0x1F6D8),
    (0x1F6DC, 0x1F6E5),
    (0x1F6E9, 0x1F6E9),
    (0x1F6EB, 0x1F6EC),
    (0x1F6F0, 0x1F6F0),
    (0x1F6F3, 0x1F6FC),
    (0x1F7E0, 0x1F7EB),
    (0x1F7F0, 0x1F7F0),
    (0x1F90C, 0x1F93A),
    (0x1F93C, 0x1F945),
    (0x1F947, 0x1F9FF),
    (0x1FA70, 0x1FA7C),
    (0x1FA80, 0x1FA8A),
    (0x1FA8E, 0x1FAC6),
    (0x1FAC8, 0x1FAC8),
    (0x1FACD, 0x1FADC),
    (0x1FADF, 0x1FAEA),
    (0x1FAEF, 0x1FAF8),
];

/// A JS `\s` character (WhiteSpace or LineTerminator), what `trim()` and
/// `/\S/` use: Unicode `White_Space` without U+0085, plus U+FEFF.
pub(crate) fn is_js_whitespace(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{FEFF}'
}

/// `$removeSegment(node, isBackward, offset)`: removes the
/// whitespace-separated segment of a segmented text node at `offset`.
fn remove_segment(node: &NodeRef, is_backward: bool, offset: usize) -> Result<(), LexicalError> {
    let text_content = node.get_text_content();
    // `textContent.split(/(?=\s)/g)`: split before each whitespace character
    // but the first.
    let mut split: Vec<&str> = Vec::new();
    let mut start = 0;
    for (i, c) in text_content.char_indices() {
        if i > 0 && is_js_whitespace(c) {
            split.push(&text_content[start..i]);
            start = i;
        }
    }
    split.push(&text_content[start..]);
    let split_length = split.len();
    let mut segment_offset = 0;
    let mut restore_offset = Some(0);
    for i in 0..split_length {
        let text = split[i];
        let is_last = i == split_length - 1;
        restore_offset = Some(segment_offset);
        segment_offset += utf16::len(text);
        if (is_backward && segment_offset == offset) || segment_offset > offset || is_last {
            split.remove(i);
            if is_last {
                restore_offset = None;
            }
            break;
        }
    }
    let joined = split.concat();
    let next_text_content = joined.trim_matches(is_js_whitespace);
    if next_text_content.is_empty() {
        node.remove(false)?;
    } else {
        node.set_text_content(next_text_content);
        node.select(restore_offset, restore_offset)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexical_selection::{Selection, create_range_selection, set_selection};
    use crate::{
        CreateEditorArgs, UpdateOptions, create_editor, json, parse_serialized_node,
        reset_random_key,
    };

    /// What JS does for one deletion on a headless editor.
    enum Want {
        /// The root's children (compact JSON) and the anchor and focus
        /// after it.
        Deleted(&'static str, &'static str, &'static str),
        /// The update throws `window object not found` and the editor keeps
        /// its state.
        NoWindow,
    }
    use Want::{Deleted, NoWindow};

    /// `deleteCharacter` ('C'), `deleteWord` ('W') and `deleteLine` ('L')
    /// on a headless editor (`createEditor` with no root element), as JS
    /// gives them in the conformance package's Node environment: anything
    /// that needs the native `Selection.modify` measurement throws, and the
    /// moves the model resolves (merging and removing blocks, removing a line
    /// break, removing an expanded selection) succeed. Points are
    /// `key:offset:t|e`.
    #[test]
    fn headless_deletion_matches_js() {
        type Case = (char, bool, usize, &'static str, &'static str, Want);
        let docs = [
            r#"[{"type":"paragraph","children":[{"type":"text","text":"abc"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","format":1,"text":"ab"},{"type":"text","text":"cd"}],"textFormat":1}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"abc"}]},{"type":"paragraph","children":[{"type":"text","text":"def"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a😀b"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"linebreak"},{"type":"text","text":"b"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"tab"},{"type":"text","text":"b"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"abcdef"}]}]"#,
            r#"[{"type":"paragraph","children":[]},{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[]},{"type":"paragraph","children":[{"type":"text","text":"b"}]}]"#,
            r#"[{"type":"paragraph","children":[]},{"type":"paragraph","children":[]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[{"type":"linebreak"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[{"type":"linebreak"},{"type":"text","text":"b"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","mode":"token","text":"ab"}]},{"type":"paragraph","children":[{"type":"text","text":"cd"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"ab"}]},{"type":"paragraph","children":[{"type":"text","mode":"segmented","text":"cd"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"text","mode":"segmented","text":"b c"},{"type":"text","text":"d"}]}]"#,
            r#"[{"type":"paragraph","children":[]}]"#,
            r#"[{"type":"paragraph","children":[]},{"type":"paragraph","children":[{"type":"text","text":"def"}]}]"#,
        ];
        let cases: &[Case] = &[
            ('C', true, 0, "2:1:t", "2:1:t", NoWindow), // mid text
            ('C', true, 0, "2:0:t", "2:0:t", NoWindow), // text start, only node
            ('C', true, 0, "2:3:t", "2:3:t", NoWindow), // text end, only node
            ('C', true, 1, "3:0:t", "3:0:t", NoWindow), // start of second text node
            ('C', true, 1, "2:2:t", "2:2:t", NoWindow), // end of first text node
            (
                'C',
                true,
                2,
                "4:0:t",
                "4:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcdef"}]}]"#,
                    "2:3:t",
                    "2:3:t",
                ),
            ), // start of second paragraph
            ('C', true, 2, "2:3:t", "2:3:t", NoWindow), // end of first paragraph
            ('C', true, 3, "2:3:t", "2:3:t", NoWindow), // after emoji
            ('C', true, 3, "2:1:t", "2:1:t", NoWindow), // before emoji
            ('C', true, 3, "2:2:t", "2:2:t", NoWindow), // inside surrogate pair
            (
                'C',
                true,
                4,
                "4:0:t",
                "4:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // text after linebreak
            ('C', true, 4, "2:1:t", "2:1:t", NoWindow), // text before linebreak
            (
                'C',
                true,
                4,
                "1:2:e",
                "1:2:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // element point after linebreak
            ('C', true, 5, "4:0:t", "4:0:t", NoWindow), // text after tab
            ('C', true, 5, "3:1:t", "3:1:t", NoWindow), // inside tab end
            ('C', true, 5, "2:1:t", "2:1:t", NoWindow), // text before tab
            (
                'C',
                true,
                6,
                "2:1:t",
                "2:4:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed
            (
                'C',
                true,
                6,
                "2:4:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed backward
            (
                'C',
                true,
                2,
                "2:1:t",
                "4:2:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"af"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed across blocks
            (
                'C',
                true,
                2,
                "2:0:t",
                "4:3:t",
                Deleted(r#"[{"type":"paragraph","children":[]}]"#, "5:0:e", "5:0:e"),
            ), // whole document
            ('C', true, 7, "1:0:e", "1:0:e", NoWindow), // empty first paragraph
            (
                'C',
                true,
                8,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // empty last paragraph
            (
                'C',
                true,
                9,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[{"type":"text","text":"b"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // empty middle paragraph
            (
                'C',
                true,
                10,
                "2:0:e",
                "2:0:e",
                Deleted(r#"[{"type":"paragraph","children":[]}]"#, "1:0:e", "1:0:e"),
            ), // two empty paragraphs
            (
                'C',
                true,
                11,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"linebreak"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // paragraph with only a linebreak
            ('C', true, 4, "1:1:e", "1:1:e", NoWindow), // element point before linebreak
            ('C', true, 12, "2:1:t", "2:1:t", NoWindow), // before a block starting with a linebreak
            (
                'C',
                true,
                13,
                "4:0:t",
                "4:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","mode":"token","text":"ab"},{"type":"text","text":"cd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // after a token block
            ('C', true, 14, "2:2:t", "2:2:t", NoWindow), // before a segmented block
            (
                'C',
                true,
                15,
                "3:2:t",
                "3:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // non-collapsed in segmented
            ('C', true, 16, "1:0:e", "1:0:e", NoWindow), // only empty paragraph
            (
                'C',
                true,
                17,
                "3:0:t",
                "3:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"def"}]}]"#,
                    "3:0:t",
                    "3:0:t",
                ),
            ), // after empty paragraph
            ('C', true, 8, "2:1:t", "2:1:t", NoWindow), // before empty paragraph
            ('C', false, 0, "2:1:t", "2:1:t", NoWindow), // mid text
            ('C', false, 0, "2:0:t", "2:0:t", NoWindow), // text start, only node
            ('C', false, 0, "2:3:t", "2:3:t", NoWindow), // text end, only node
            ('C', false, 1, "3:0:t", "3:0:t", NoWindow), // start of second text node
            ('C', false, 1, "2:2:t", "2:2:t", NoWindow), // end of first text node
            ('C', false, 2, "4:0:t", "4:0:t", NoWindow), // start of second paragraph
            (
                'C',
                false,
                2,
                "2:3:t",
                "2:3:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcdef"}]}]"#,
                    "2:3:t",
                    "2:3:t",
                ),
            ), // end of first paragraph
            ('C', false, 3, "2:3:t", "2:3:t", NoWindow), // after emoji
            ('C', false, 3, "2:1:t", "2:1:t", NoWindow), // before emoji
            ('C', false, 3, "2:2:t", "2:2:t", NoWindow), // inside surrogate pair
            ('C', false, 4, "4:0:t", "4:0:t", NoWindow), // text after linebreak
            (
                'C',
                false,
                4,
                "2:1:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // text before linebreak
            ('C', false, 4, "1:2:e", "1:2:e", NoWindow), // element point after linebreak
            ('C', false, 5, "4:0:t", "4:0:t", NoWindow), // text after tab
            ('C', false, 5, "3:1:t", "3:1:t", NoWindow), // inside tab end
            ('C', false, 5, "2:1:t", "2:1:t", NoWindow), // text before tab
            (
                'C',
                false,
                6,
                "2:1:t",
                "2:4:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed
            (
                'C',
                false,
                6,
                "2:4:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed backward
            (
                'C',
                false,
                2,
                "2:1:t",
                "4:2:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"af"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed across blocks
            (
                'C',
                false,
                2,
                "2:0:t",
                "4:3:t",
                Deleted(r#"[{"type":"paragraph","children":[]}]"#, "5:0:e", "5:0:e"),
            ), // whole document
            (
                'C',
                false,
                7,
                "1:0:e",
                "1:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
                    "3:0:t",
                    "3:0:t",
                ),
            ), // empty first paragraph
            ('C', false, 8, "3:0:e", "3:0:e", NoWindow), // empty last paragraph
            (
                'C',
                false,
                9,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[{"type":"text","text":"b"}]}]"#,
                    "root:1:e",
                    "root:1:e",
                ),
            ), // empty middle paragraph
            ('C', false, 10, "2:0:e", "2:0:e", NoWindow), // two empty paragraphs
            (
                'C',
                false,
                11,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[]}]"#,
                    "3:0:e",
                    "3:0:e",
                ),
            ), // paragraph with only a linebreak
            (
                'C',
                false,
                4,
                "1:1:e",
                "1:1:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // element point before linebreak
            (
                'C',
                false,
                12,
                "2:1:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"linebreak"},{"type":"text","text":"b"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // before a block starting with a linebreak
            ('C', false, 13, "4:0:t", "4:0:t", NoWindow), // after a token block
            (
                'C',
                false,
                14,
                "2:2:t",
                "2:2:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"ab"},{"type":"text","mode":"segmented","text":"cd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // before a segmented block
            (
                'C',
                false,
                15,
                "3:2:t",
                "3:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // non-collapsed in segmented
            ('C', false, 16, "1:0:e", "1:0:e", NoWindow), // only empty paragraph
            ('C', false, 17, "3:0:t", "3:0:t", NoWindow), // after empty paragraph
            (
                'C',
                false,
                8,
                "2:1:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // before empty paragraph
            ('W', true, 0, "2:1:t", "2:1:t", NoWindow), // mid text
            ('W', true, 0, "2:0:t", "2:0:t", NoWindow), // text start, only node
            (
                'W',
                true,
                2,
                "4:0:t",
                "4:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcdef"}]}]"#,
                    "2:3:t",
                    "2:3:t",
                ),
            ), // start of second paragraph
            ('W', true, 4, "2:1:t", "2:1:t", NoWindow), // text before linebreak
            (
                'W',
                true,
                6,
                "2:1:t",
                "2:4:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed
            (
                'W',
                true,
                6,
                "2:4:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed backward
            (
                'W',
                true,
                2,
                "2:1:t",
                "4:2:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"af"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed across blocks
            (
                'W',
                true,
                2,
                "2:0:t",
                "4:3:t",
                Deleted(r#"[{"type":"paragraph","children":[]}]"#, "5:0:e", "5:0:e"),
            ), // whole document
            (
                'W',
                true,
                8,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // empty last paragraph
            (
                'W',
                true,
                9,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[{"type":"text","text":"b"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // empty middle paragraph
            (
                'W',
                true,
                10,
                "2:0:e",
                "2:0:e",
                Deleted(r#"[{"type":"paragraph","children":[]}]"#, "1:0:e", "1:0:e"),
            ), // two empty paragraphs
            (
                'W',
                true,
                11,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"linebreak"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // paragraph with only a linebreak
            (
                'W',
                true,
                13,
                "4:0:t",
                "4:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","mode":"token","text":"ab"},{"type":"text","text":"cd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // after a token block
            (
                'W',
                true,
                15,
                "3:2:t",
                "3:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // non-collapsed in segmented
            (
                'W',
                true,
                17,
                "3:0:t",
                "3:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"def"}]}]"#,
                    "3:0:t",
                    "3:0:t",
                ),
            ), // after empty paragraph
            ('W', false, 0, "2:1:t", "2:1:t", NoWindow), // mid text
            ('W', false, 0, "2:0:t", "2:0:t", NoWindow), // text start, only node
            (
                'W',
                false,
                2,
                "2:3:t",
                "2:3:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcdef"}]}]"#,
                    "2:3:t",
                    "2:3:t",
                ),
            ), // end of first paragraph
            ('W', false, 4, "2:1:t", "2:1:t", NoWindow), // text before linebreak
            (
                'W',
                false,
                6,
                "2:1:t",
                "2:4:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed
            (
                'W',
                false,
                6,
                "2:4:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed backward
            (
                'W',
                false,
                2,
                "2:1:t",
                "4:2:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"af"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed across blocks
            (
                'W',
                false,
                2,
                "2:0:t",
                "4:3:t",
                Deleted(r#"[{"type":"paragraph","children":[]}]"#, "5:0:e", "5:0:e"),
            ), // whole document
            (
                'W',
                false,
                7,
                "1:0:e",
                "1:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
                    "3:0:t",
                    "3:0:t",
                ),
            ), // empty first paragraph
            (
                'W',
                false,
                9,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[{"type":"text","text":"b"}]}]"#,
                    "5:0:t",
                    "5:0:t",
                ),
            ), // empty middle paragraph
            (
                'W',
                false,
                12,
                "2:1:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"linebreak"},{"type":"text","text":"b"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // before a block starting with a linebreak
            (
                'W',
                false,
                14,
                "2:2:t",
                "2:2:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"ab"},{"type":"text","mode":"segmented","text":"cd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // before a segmented block
            (
                'W',
                false,
                15,
                "3:2:t",
                "3:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // non-collapsed in segmented
            (
                'W',
                false,
                8,
                "2:1:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // before empty paragraph
            ('L', true, 0, "2:1:t", "2:1:t", NoWindow), // mid text
            ('L', true, 0, "2:0:t", "2:0:t", NoWindow), // text start, only node
            (
                'L',
                true,
                2,
                "4:0:t",
                "4:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcdef"}]}]"#,
                    "2:3:t",
                    "2:3:t",
                ),
            ), // start of second paragraph
            ('L', true, 4, "2:1:t", "2:1:t", NoWindow), // text before linebreak
            (
                'L',
                true,
                6,
                "2:1:t",
                "2:4:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed
            (
                'L',
                true,
                6,
                "2:4:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed backward
            (
                'L',
                true,
                8,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // empty last paragraph
            (
                'L',
                true,
                9,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[{"type":"text","text":"b"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // empty middle paragraph
            (
                'L',
                true,
                10,
                "2:0:e",
                "2:0:e",
                Deleted(r#"[{"type":"paragraph","children":[]}]"#, "1:0:e", "1:0:e"),
            ), // two empty paragraphs
            (
                'L',
                true,
                11,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"linebreak"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // paragraph with only a linebreak
            (
                'L',
                true,
                13,
                "4:0:t",
                "4:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","mode":"token","text":"ab"},{"type":"text","text":"cd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // after a token block
            (
                'L',
                true,
                15,
                "3:2:t",
                "3:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // non-collapsed in segmented
            (
                'L',
                true,
                17,
                "3:0:t",
                "3:0:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"def"}]}]"#,
                    "3:0:t",
                    "3:0:t",
                ),
            ), // after empty paragraph
            ('L', false, 0, "2:1:t", "2:1:t", NoWindow), // mid text
            ('L', false, 0, "2:0:t", "2:0:t", NoWindow), // text start, only node
            (
                'L',
                false,
                2,
                "2:3:t",
                "2:3:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcdef"}]}]"#,
                    "2:3:t",
                    "2:3:t",
                ),
            ), // end of first paragraph
            ('L', false, 4, "2:1:t", "2:1:t", NoWindow), // text before linebreak
            (
                'L',
                false,
                6,
                "2:1:t",
                "2:4:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed
            (
                'L',
                false,
                6,
                "2:4:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"aef"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // non-collapsed backward
            (
                'L',
                false,
                7,
                "1:0:e",
                "1:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
                    "3:0:t",
                    "3:0:t",
                ),
            ), // empty first paragraph
            (
                'L',
                false,
                9,
                "3:0:e",
                "3:0:e",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]},{"type":"paragraph","children":[{"type":"text","text":"b"}]}]"#,
                    "root:1:e",
                    "root:1:e",
                ),
            ), // empty middle paragraph
            (
                'L',
                false,
                12,
                "2:1:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"linebreak"},{"type":"text","text":"b"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // before a block starting with a linebreak
            (
                'L',
                false,
                14,
                "2:2:t",
                "2:2:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"ab"},{"type":"text","mode":"segmented","text":"cd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // before a segmented block
            (
                'L',
                false,
                15,
                "3:2:t",
                "3:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"abcd"}]}]"#,
                    "2:2:t",
                    "2:2:t",
                ),
            ), // non-collapsed in segmented
            (
                'L',
                false,
                8,
                "2:1:t",
                "2:1:t",
                Deleted(
                    r#"[{"type":"paragraph","children":[{"type":"text","text":"a"}]}]"#,
                    "2:1:t",
                    "2:1:t",
                ),
            ), // before empty paragraph
        ];
        let point = |spec: &str| {
            let mut parts = spec.split(':');
            let key = parts.next().unwrap().to_owned();
            let offset: usize = parts.next().unwrap().parse().unwrap();
            let point_type = match parts.next().unwrap() {
                "t" => PointType::Text,
                _ => PointType::Element,
            };
            Point::new(key, offset, point_type)
        };
        let show = |p: &Point| {
            let t = match p.point_type {
                PointType::Text => "t",
                PointType::Element => "e",
            };
            format!("{}:{}:{t}", p.key, p.offset)
        };
        let discrete = || UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        };
        for (method, is_backward, doc, anchor, focus, want) in cases {
            let label = format!("{method} {is_backward} doc {doc} {anchor} {focus}");
            reset_random_key();
            let editor = create_editor(CreateEditorArgs::default());
            let children = json::parse(docs[*doc]).unwrap();
            editor
                .update(
                    move || {
                        let nodes: Vec<NodeRef> = children
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|n| parse_serialized_node(n).unwrap())
                            .collect();
                        get_root().append(&nodes)
                    },
                    discrete(),
                )
                .unwrap();
            let before = json::stringify(&editor.get_editor_state().to_json(true));
            let (anchor, focus, method, is_backward) =
                (point(anchor), point(focus), *method, *is_backward);
            let result = editor.update(
                move || {
                    let selection = create_range_selection();
                    selection.point_set(
                        Which::Anchor,
                        &anchor.key,
                        anchor.offset,
                        anchor.point_type,
                    )?;
                    selection.point_set(
                        Which::Focus,
                        &focus.key,
                        focus.offset,
                        focus.point_type,
                    )?;
                    set_selection(Some(selection.clone()));
                    match method {
                        'C' => selection.delete_character(is_backward),
                        'W' => selection.delete_word(is_backward),
                        _ => selection.delete_line(is_backward),
                    }
                },
                discrete(),
            );
            let state = editor.get_editor_state();
            let compact = json::stringify(&state.to_json(true)).unwrap();
            match want {
                NoWindow => {
                    assert_eq!(
                        result.map_err(|e| e.message().to_owned()),
                        Err("window object not found".to_owned()),
                        "{label}"
                    );
                    assert_eq!(Some(compact), before, "{label}");
                }
                Deleted(children, want_anchor, want_focus) => {
                    assert_eq!(result, Ok(()), "{label}");
                    assert_eq!(
                        compact,
                        format!(r#"{{"root":{{"type":"root","children":{children}}}}}"#),
                        "{label}"
                    );
                    let Some(Selection::Range(range)) = state.selection().map(|s| s.get()) else {
                        panic!("{label}: no range selection");
                    };
                    assert_eq!(
                        (show(&range.anchor), show(&range.focus)),
                        (want_anchor.to_string(), want_focus.to_string()),
                        "{label}"
                    );
                }
            }
        }
    }

    /// A [`SelectionMovement`] that never leaves a text node: a character is
    /// a code point with the combining marks (U+0300 to U+036F) and variation
    /// selector 16 after it, a word is the spaces and then the non-spaces in
    /// the direction, and a line boundary is the text node's edge. The JS
    /// expectations below come from stubbing `Selection.prototype.modify`
    /// with the same rules in jsdom.
    struct TestMovement;

    impl SelectionMovement for TestMovement {
        fn landed_point(
            &self,
            from: &Point,
            is_backward: bool,
            granularity: SelectionGranularity,
        ) -> Result<Option<Point>, LexicalError> {
            if from.point_type != PointType::Text {
                return Ok(None);
            }
            let text: Vec<u16> = from.get_node()?.get_text_content().encode_utf16().collect();
            let len = text.len();
            let is_mark = |c: u16| (0x300..=0x36f).contains(&c) || c == 0xfe0f;
            let is_high = |c: u16| (0xd800..=0xdbff).contains(&c);
            let is_low = |c: u16| (0xdc00..=0xdfff).contains(&c);
            let is_space = |c: u16| c == u16::from(b' ');
            let mut o = from.offset;
            match (granularity, is_backward) {
                (SelectionGranularity::Character, true) => {
                    while o > 0 && is_mark(text[o - 1]) {
                        o -= 1;
                    }
                    if o > 0 {
                        o -= 1;
                        if o > 0 && is_low(text[o]) && is_high(text[o - 1]) {
                            o -= 1;
                        }
                    }
                }
                (SelectionGranularity::Character, false) => {
                    if o < len {
                        o += if is_high(text[o]) && o + 1 < len && is_low(text[o + 1]) {
                            2
                        } else {
                            1
                        };
                        while o < len && is_mark(text[o]) {
                            o += 1;
                        }
                    }
                }
                (SelectionGranularity::Word, true) => {
                    while o > 0 && is_space(text[o - 1]) {
                        o -= 1;
                    }
                    while o > 0 && !is_space(text[o - 1]) {
                        o -= 1;
                    }
                }
                (SelectionGranularity::Word, false) => {
                    while o < len && is_space(text[o]) {
                        o += 1;
                    }
                    while o < len && !is_space(text[o]) {
                        o += 1;
                    }
                }
                (_, true) => o = 0,
                (_, false) => o = len,
            }
            Ok((o != from.offset).then(|| Point::new(from.key.clone(), o, PointType::Text)))
        }
    }

    /// `key:offset:t|e` as a point.
    fn parse_point(spec: &str) -> Point {
        let mut parts = spec.split(':');
        let key = parts.next().unwrap().to_owned();
        let offset = parts.next().unwrap().parse().unwrap();
        let point_type = match parts.next().unwrap() {
            "t" => PointType::Text,
            _ => PointType::Element,
        };
        Point::new(key, offset, point_type)
    }

    fn show_point(point: &Point) -> String {
        let t = match point.point_type {
            PointType::Text => "t",
            PointType::Element => "e",
        };
        format!("{}:{}:{t}", point.key, point.offset)
    }

    /// `deleteCharacter`, `deleteWord`, `deleteLine` (`C`, `W`, `L`, `<`
    /// backward and `>` forward) and `modify` (`move` or `extend`, the
    /// direction, then `c`haracter, `w`ord or `l`ineboundary) with
    /// [`TestMovement`], as JS gives them for an editor with a root element in
    /// jsdom and `Selection.prototype.modify` stubbed to move the same way.
    /// Each line: the operation, the document, anchor and focus | the anchor,
    /// focus and format after it | the root's children after it (`=` when
    /// unchanged).
    #[test]
    fn deletion_and_modify_with_a_movement_match_js() {
        let docs = [
            r#"[{"type":"paragraph","children":[{"type":"text","text":"hello big world"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a😀b"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"aéb"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"a❤️b"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","format":1,"text":"ab"},{"type":"text","text":"cd"}],"textFormat":1}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","detail":2,"text":"ab"},{"type":"text","detail":2,"text":"cd"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"ab cd"},{"type":"linebreak"},{"type":"text","text":"ef gh"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[{"type":"text","text":"ef gh"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"x "},{"type":"text","mode":"segmented","text":"foo bar baz"},{"type":"text","text":" y"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"x "},{"type":"text","mode":"token","text":"tok"},{"type":"text","text":" y"}]}]"#,
            r#"[{"type":"paragraph","children":[{"type":"text","text":"ab"}]},{"type":"paragraph","children":[]}]"#,
        ];
        let cases = r#"
C< 0 2:7:t 2:7:t | 2:6:t 2:6:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello ig world"}]}]
W< 0 2:7:t 2:7:t | 2:6:t 2:6:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello ig world"}]}]
L< 0 2:7:t 2:7:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ig world"}]}]
C> 0 2:7:t 2:7:t | 2:7:t 2:7:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello bg world"}]}]
W> 0 2:7:t 2:7:t | 2:7:t 2:7:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello b world"}]}]
L> 0 2:7:t 2:7:t | 2:7:t 2:7:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello b"}]}]
move<c 0 2:7:t 2:7:t | 2:6:t 2:6:t 0 | =
move<w 0 2:7:t 2:7:t | 2:6:t 2:6:t 0 | =
move<l 0 2:7:t 2:7:t | 2:0:t 2:0:t 0 | =
move>c 0 2:7:t 2:7:t | 2:8:t 2:8:t 0 | =
move>w 0 2:7:t 2:7:t | 2:9:t 2:9:t 0 | =
move>l 0 2:7:t 2:7:t | 2:15:t 2:15:t 0 | =
extend<c 0 2:7:t 2:7:t | 2:7:t 2:6:t 0 | =
extend<w 0 2:7:t 2:7:t | 2:7:t 2:6:t 0 | =
extend<l 0 2:7:t 2:7:t | 2:7:t 2:0:t 0 | =
extend>c 0 2:7:t 2:7:t | 2:7:t 2:8:t 0 | =
extend>w 0 2:7:t 2:7:t | 2:7:t 2:9:t 0 | =
extend>l 0 2:7:t 2:7:t | 2:7:t 2:15:t 0 | =
C< 0 2:6:t 2:6:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hellobig world"}]}]
W< 0 2:6:t 2:6:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"big world"}]}]
L< 0 2:6:t 2:6:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"big world"}]}]
C> 0 2:6:t 2:6:t | 2:6:t 2:6:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello ig world"}]}]
W> 0 2:6:t 2:6:t | 2:6:t 2:6:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello  world"}]}]
L> 0 2:6:t 2:6:t | 2:6:t 2:6:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello "}]}]
C< 0 2:5:t 2:5:t | 2:4:t 2:4:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hell big world"}]}]
W< 0 2:5:t 2:5:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":" big world"}]}]
L< 0 2:5:t 2:5:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":" big world"}]}]
C> 0 2:5:t 2:5:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hellobig world"}]}]
W> 0 2:5:t 2:5:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello world"}]}]
L> 0 2:5:t 2:5:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello"}]}]
C< 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | =
W< 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | =
L< 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | =
C> 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ello big world"}]}]
W> 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":" big world"}]}]
L> 0 2:0:t 2:0:t | 1:0:e 1:0:e 0 | [{"type":"paragraph","children":[]}]
move<c 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | =
move<w 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | =
move<l 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | =
move>c 0 2:0:t 2:0:t | 2:1:t 2:1:t 0 | =
move>w 0 2:0:t 2:0:t | 2:5:t 2:5:t 0 | =
move>l 0 2:0:t 2:0:t | 2:15:t 2:15:t 0 | =
extend<c 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | =
extend<w 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | =
extend<l 0 2:0:t 2:0:t | 2:0:t 2:0:t 0 | =
extend>c 0 2:0:t 2:0:t | 2:0:t 2:1:t 0 | =
extend>w 0 2:0:t 2:0:t | 2:0:t 2:5:t 0 | =
extend>l 0 2:0:t 2:0:t | 2:0:t 2:15:t 0 | =
C< 0 2:15:t 2:15:t | 2:14:t 2:14:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello big worl"}]}]
W< 0 2:15:t 2:15:t | 2:10:t 2:10:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"hello big "}]}]
L< 0 2:15:t 2:15:t | 1:0:e 1:0:e 0 | [{"type":"paragraph","children":[]}]
C> 0 2:15:t 2:15:t | 2:15:t 2:15:t 0 | =
W> 0 2:15:t 2:15:t | 2:15:t 2:15:t 0 | =
L> 0 2:15:t 2:15:t | 2:15:t 2:15:t 0 | =
move<c 0 2:15:t 2:15:t | 2:14:t 2:14:t 0 | =
move<w 0 2:15:t 2:15:t | 2:10:t 2:10:t 0 | =
move<l 0 2:15:t 2:15:t | 2:0:t 2:0:t 0 | =
move>c 0 2:15:t 2:15:t | 2:15:t 2:15:t 0 | =
move>w 0 2:15:t 2:15:t | 2:15:t 2:15:t 0 | =
move>l 0 2:15:t 2:15:t | 2:15:t 2:15:t 0 | =
extend<c 0 2:15:t 2:15:t | 2:15:t 2:14:t 0 | =
extend<w 0 2:15:t 2:15:t | 2:15:t 2:10:t 0 | =
extend<l 0 2:15:t 2:15:t | 2:15:t 2:0:t 0 | =
extend>c 0 2:15:t 2:15:t | 2:15:t 2:15:t 0 | =
extend>w 0 2:15:t 2:15:t | 2:15:t 2:15:t 0 | =
extend>l 0 2:15:t 2:15:t | 2:15:t 2:15:t 0 | =
move<c 0 2:2:t 2:8:t | 2:2:t 2:2:t 0 | =
move<w 0 2:2:t 2:8:t | 2:6:t 2:6:t 0 | =
move<l 0 2:2:t 2:8:t | 2:0:t 2:0:t 0 | =
move>c 0 2:2:t 2:8:t | 2:8:t 2:8:t 0 | =
move>w 0 2:2:t 2:8:t | 2:9:t 2:9:t 0 | =
move>l 0 2:2:t 2:8:t | 2:15:t 2:15:t 0 | =
extend<c 0 2:2:t 2:8:t | 2:2:t 2:7:t 0 | =
extend<w 0 2:2:t 2:8:t | 2:2:t 2:6:t 0 | =
extend<l 0 2:2:t 2:8:t | 2:2:t 2:0:t 0 | =
extend>c 0 2:2:t 2:8:t | 2:2:t 2:9:t 0 | =
extend>w 0 2:2:t 2:8:t | 2:2:t 2:9:t 0 | =
extend>l 0 2:2:t 2:8:t | 2:2:t 2:15:t 0 | =
move<c 0 2:8:t 2:2:t | 2:2:t 2:2:t 0 | =
move<w 0 2:8:t 2:2:t | 2:0:t 2:0:t 0 | =
move<l 0 2:8:t 2:2:t | 2:0:t 2:0:t 0 | =
move>c 0 2:8:t 2:2:t | 2:8:t 2:8:t 0 | =
move>w 0 2:8:t 2:2:t | 2:5:t 2:5:t 0 | =
move>l 0 2:8:t 2:2:t | 2:15:t 2:15:t 0 | =
extend<c 0 2:8:t 2:2:t | 2:8:t 2:1:t 0 | =
extend<w 0 2:8:t 2:2:t | 2:8:t 2:0:t 0 | =
extend<l 0 2:8:t 2:2:t | 2:8:t 2:0:t 0 | =
extend>c 0 2:8:t 2:2:t | 2:8:t 2:3:t 0 | =
extend>w 0 2:8:t 2:2:t | 2:8:t 2:5:t 0 | =
extend>l 0 2:8:t 2:2:t | 2:8:t 2:15:t 0 | =
C< 1 2:3:t 2:3:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]
W< 1 2:3:t 2:3:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"b"}]}]
L< 1 2:3:t 2:3:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"b"}]}]
C> 1 2:1:t 2:1:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]
W> 1 2:1:t 2:1:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"a"}]}]
L> 1 2:1:t 2:1:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"a"}]}]
C< 2 2:3:t 2:3:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"aeb"}]}]
W< 2 2:3:t 2:3:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"b"}]}]
L< 2 2:3:t 2:3:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"b"}]}]
C> 2 2:1:t 2:1:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"aeb"}]}]
W> 2 2:1:t 2:1:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"a"}]}]
L> 2 2:1:t 2:1:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"a"}]}]
C< 3 2:3:t 2:3:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]
W< 3 2:3:t 2:3:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"b"}]}]
L< 3 2:3:t 2:3:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"b"}]}]
C> 3 2:1:t 2:1:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]
W> 3 2:1:t 2:1:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"a"}]}]
L> 3 2:1:t 2:1:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"a"}]}]
C< 4 2:2:t 2:2:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","format":1,"text":"a"},{"type":"text","text":"cd"}],"textFormat":1}]
W< 4 2:2:t 2:2:t | 3:0:t 3:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"cd"}]}]
L< 4 2:2:t 2:2:t | 3:0:t 3:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"cd"}]}]
C> 4 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
W> 4 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
L> 4 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
move<c 4 2:2:t 2:2:t | 2:1:t 2:1:t 0 | =
move<w 4 2:2:t 2:2:t | 2:0:t 2:0:t 0 | =
move<l 4 2:2:t 2:2:t | 2:0:t 2:0:t 0 | =
move>c 4 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
move>w 4 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
move>l 4 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
extend<c 4 2:2:t 2:2:t | 2:2:t 2:1:t 0 | =
extend<w 4 2:2:t 2:2:t | 2:2:t 2:0:t 0 | =
extend<l 4 2:2:t 2:2:t | 2:2:t 2:0:t 0 | =
extend>c 4 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
extend>w 4 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
extend>l 4 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
C< 4 3:0:t 3:0:t | 3:0:t 3:0:t 0 | =
W< 4 3:0:t 3:0:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","format":1,"text":"a"},{"type":"text","text":"cd"}],"textFormat":1}]
L< 4 3:0:t 3:0:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","format":1,"text":"a"},{"type":"text","text":"cd"}],"textFormat":1}]
C> 4 3:0:t 3:0:t | 3:0:t 3:0:t 0 | [{"type":"paragraph","children":[{"type":"text","format":1,"text":"ab"},{"type":"text","text":"d"}],"textFormat":1}]
W> 4 3:0:t 3:0:t | 2:2:t 2:2:t 1 | [{"type":"paragraph","children":[{"type":"text","format":1,"text":"ab"}],"textFormat":1}]
L> 4 3:0:t 3:0:t | 2:2:t 2:2:t 1 | [{"type":"paragraph","children":[{"type":"text","format":1,"text":"ab"}],"textFormat":1}]
move<c 4 3:0:t 3:0:t | 2:2:t 2:2:t 0 | =
move<w 4 3:0:t 3:0:t | 2:2:t 2:2:t 0 | =
move<l 4 3:0:t 3:0:t | 2:2:t 2:2:t 0 | =
move>c 4 3:0:t 3:0:t | 3:1:t 3:1:t 0 | =
move>w 4 3:0:t 3:0:t | 3:2:t 3:2:t 0 | =
move>l 4 3:0:t 3:0:t | 3:2:t 3:2:t 0 | =
extend<c 4 3:0:t 3:0:t | 2:2:t 2:2:t 0 | =
extend<w 4 3:0:t 3:0:t | 2:2:t 2:2:t 0 | =
extend<l 4 3:0:t 3:0:t | 2:2:t 2:2:t 0 | =
extend>c 4 3:0:t 3:0:t | 3:0:t 3:1:t 0 | =
extend>w 4 3:0:t 3:0:t | 3:0:t 3:2:t 0 | =
extend>l 4 3:0:t 3:0:t | 3:0:t 3:2:t 0 | =
C< 5 2:2:t 2:2:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"a"},{"type":"text","detail":2,"text":"cd"}]}]
W< 5 2:2:t 2:2:t | 3:0:t 3:0:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"cd"}]}]
L< 5 2:2:t 2:2:t | 3:0:t 3:0:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"cd"}]}]
C> 5 2:2:t 2:2:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"ab"},{"type":"text","detail":2,"text":"d"}]}]
W> 5 2:2:t 2:2:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"ab"},{"type":"text","detail":2,"text":"d"}]}]
L> 5 2:2:t 2:2:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"ab"},{"type":"text","detail":2,"text":"d"}]}]
move<c 5 2:2:t 2:2:t | 2:1:t 2:1:t 0 | =
move<w 5 2:2:t 2:2:t | 2:0:t 2:0:t 0 | =
move<l 5 2:2:t 2:2:t | 2:0:t 2:0:t 0 | =
move>c 5 2:2:t 2:2:t | 3:1:t 3:1:t 0 | =
move>w 5 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
move>l 5 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
extend<c 5 2:2:t 2:2:t | 2:2:t 2:1:t 0 | =
extend<w 5 2:2:t 2:2:t | 2:2:t 2:0:t 0 | =
extend<l 5 2:2:t 2:2:t | 2:2:t 2:0:t 0 | =
extend>c 5 2:2:t 2:2:t | 2:2:t 3:1:t 0 | =
extend>w 5 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
extend>l 5 2:2:t 2:2:t | 2:2:t 2:2:t 0 | =
C< 5 3:0:t 3:0:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"a"},{"type":"text","detail":2,"text":"cd"}]}]
W< 5 3:0:t 3:0:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"a"},{"type":"text","detail":2,"text":"cd"}]}]
L< 5 3:0:t 3:0:t | 2:1:t 2:1:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"a"},{"type":"text","detail":2,"text":"cd"}]}]
C> 5 3:0:t 3:0:t | 3:0:t 3:0:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"ab"},{"type":"text","detail":2,"text":"d"}]}]
W> 5 3:0:t 3:0:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"ab"}]}]
L> 5 3:0:t 3:0:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","detail":2,"text":"ab"}]}]
move<c 5 3:0:t 3:0:t | 2:1:t 2:1:t 0 | =
move<w 5 3:0:t 3:0:t | 3:0:t 3:0:t 0 | =
move<l 5 3:0:t 3:0:t | 3:0:t 3:0:t 0 | =
move>c 5 3:0:t 3:0:t | 3:1:t 3:1:t 0 | =
move>w 5 3:0:t 3:0:t | 3:2:t 3:2:t 0 | =
move>l 5 3:0:t 3:0:t | 3:2:t 3:2:t 0 | =
extend<c 5 3:0:t 3:0:t | 3:0:t 2:1:t 0 | =
extend<w 5 3:0:t 3:0:t | 3:0:t 3:0:t 0 | =
extend<l 5 3:0:t 3:0:t | 3:0:t 3:0:t 0 | =
extend>c 5 3:0:t 3:0:t | 3:0:t 3:1:t 0 | =
extend>w 5 3:0:t 3:0:t | 3:0:t 3:2:t 0 | =
extend>l 5 3:0:t 3:0:t | 3:0:t 3:2:t 0 | =
C< 6 4:3:t 4:3:t | 4:2:t 4:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"},{"type":"linebreak"},{"type":"text","text":"efgh"}]}]
W< 6 4:3:t 4:3:t | 4:0:t 4:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"},{"type":"linebreak"},{"type":"text","text":"gh"}]}]
L< 6 4:3:t 4:3:t | 4:0:t 4:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"},{"type":"linebreak"},{"type":"text","text":"gh"}]}]
C> 6 4:3:t 4:3:t | 4:3:t 4:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"},{"type":"linebreak"},{"type":"text","text":"ef h"}]}]
W> 6 4:3:t 4:3:t | 4:3:t 4:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"},{"type":"linebreak"},{"type":"text","text":"ef "}]}]
L> 6 4:3:t 4:3:t | 4:3:t 4:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"},{"type":"linebreak"},{"type":"text","text":"ef "}]}]
C< 6 2:3:t 2:3:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"abcd"},{"type":"linebreak"},{"type":"text","text":"ef gh"}]}]
W< 6 2:3:t 2:3:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"cd"},{"type":"linebreak"},{"type":"text","text":"ef gh"}]}]
L< 6 2:3:t 2:3:t | 2:0:t 2:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"cd"},{"type":"linebreak"},{"type":"text","text":"ef gh"}]}]
C> 6 2:3:t 2:3:t | 2:3:t 2:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab d"},{"type":"linebreak"},{"type":"text","text":"ef gh"}]}]
W> 6 2:3:t 2:3:t | 2:3:t 2:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab "},{"type":"linebreak"},{"type":"text","text":"ef gh"}]}]
L> 6 2:3:t 2:3:t | 2:3:t 2:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab "},{"type":"linebreak"},{"type":"text","text":"ef gh"}]}]
C< 6 1:2:e 1:2:e | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cdef gh"}]}]
W< 6 1:2:e 1:2:e | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cdef gh"}]}]
L< 6 1:2:e 1:2:e | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cdef gh"}]}]
C> 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
W> 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"},{"type":"linebreak"},{"type":"text","text":"f gh"}]}]
L> 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"},{"type":"linebreak"},{"type":"text","text":"f gh"}]}]
move<c 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
move<w 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
move<l 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
move>c 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
move>w 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
move>l 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
extend<c 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
extend<w 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
extend<l 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
extend>c 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
extend>w 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
extend>l 6 1:2:e 1:2:e | 4:0:t 4:0:t 0 | =
C< 7 4:0:t 4:0:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cdef gh"}]}]
W< 7 4:0:t 4:0:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cdef gh"}]}]
L< 7 4:0:t 4:0:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cdef gh"}]}]
C> 7 4:0:t 4:0:t | 4:0:t 4:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[{"type":"text","text":"f gh"}]}]
W> 7 4:0:t 4:0:t | 4:0:t 4:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[{"type":"text","text":" gh"}]}]
L> 7 4:0:t 4:0:t | 3:0:e 3:0:e 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[]}]
move<c 7 4:0:t 4:0:t | 2:5:t 2:5:t 0 | =
move<w 7 4:0:t 4:0:t | 2:5:t 2:5:t 0 | =
move<l 7 4:0:t 4:0:t | 2:5:t 2:5:t 0 | =
move>c 7 4:0:t 4:0:t | 4:1:t 4:1:t 0 | =
move>w 7 4:0:t 4:0:t | 4:2:t 4:2:t 0 | =
move>l 7 4:0:t 4:0:t | 4:5:t 4:5:t 0 | =
extend<c 7 4:0:t 4:0:t | 4:0:t 2:5:t 0 | =
extend<w 7 4:0:t 4:0:t | 4:0:t 2:5:t 0 | =
extend<l 7 4:0:t 4:0:t | 4:0:t 2:5:t 0 | =
extend>c 7 4:0:t 4:0:t | 4:0:t 4:1:t 0 | =
extend>w 7 4:0:t 4:0:t | 4:0:t 4:2:t 0 | =
extend>l 7 4:0:t 4:0:t | 4:0:t 4:5:t 0 | =
C< 7 2:5:t 2:5:t | 2:4:t 2:4:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab c"}]},{"type":"paragraph","children":[{"type":"text","text":"ef gh"}]}]
W< 7 2:5:t 2:5:t | 2:3:t 2:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab "}]},{"type":"paragraph","children":[{"type":"text","text":"ef gh"}]}]
L< 7 2:5:t 2:5:t | 1:0:e 1:0:e 0 | [{"type":"paragraph","children":[]},{"type":"paragraph","children":[{"type":"text","text":"ef gh"}]}]
C> 7 2:5:t 2:5:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cdef gh"}]}]
W> 7 2:5:t 2:5:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cdef gh"}]}]
L> 7 2:5:t 2:5:t | 2:5:t 2:5:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cdef gh"}]}]
move<c 7 2:5:t 2:5:t | 2:4:t 2:4:t 0 | =
move<w 7 2:5:t 2:5:t | 2:3:t 2:3:t 0 | =
move<l 7 2:5:t 2:5:t | 2:0:t 2:0:t 0 | =
move>c 7 2:5:t 2:5:t | 4:0:t 4:0:t 0 | =
move>w 7 2:5:t 2:5:t | 4:0:t 4:0:t 0 | =
move>l 7 2:5:t 2:5:t | 4:0:t 4:0:t 0 | =
extend<c 7 2:5:t 2:5:t | 2:5:t 2:4:t 0 | =
extend<w 7 2:5:t 2:5:t | 2:5:t 2:3:t 0 | =
extend<l 7 2:5:t 2:5:t | 2:5:t 2:0:t 0 | =
extend>c 7 2:5:t 2:5:t | 2:5:t 4:0:t 0 | =
extend>w 7 2:5:t 2:5:t | 2:5:t 4:0:t 0 | =
extend>l 7 2:5:t 2:5:t | 2:5:t 4:0:t 0 | =
C< 7 4:4:t 4:4:t | 4:3:t 4:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[{"type":"text","text":"ef h"}]}]
W< 7 4:4:t 4:4:t | 4:3:t 4:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[{"type":"text","text":"ef h"}]}]
L< 7 4:4:t 4:4:t | 4:0:t 4:0:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[{"type":"text","text":"h"}]}]
C> 7 4:4:t 4:4:t | 4:4:t 4:4:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[{"type":"text","text":"ef g"}]}]
W> 7 4:4:t 4:4:t | 4:4:t 4:4:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[{"type":"text","text":"ef g"}]}]
L> 7 4:4:t 4:4:t | 4:4:t 4:4:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab cd"}]},{"type":"paragraph","children":[{"type":"text","text":"ef g"}]}]
C< 8 3:5:t 3:5:t | 3:3:t 3:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x "},{"type":"text","mode":"segmented","text":"foo baz"},{"type":"text","text":" y"}]}]
W< 8 3:5:t 3:5:t | 2:6:t 2:6:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x foo ar baz y"}]}]
L< 8 3:5:t 3:5:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x ar baz y"}]}]
C> 8 3:5:t 3:5:t | 3:3:t 3:3:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x "},{"type":"text","mode":"segmented","text":"foo baz"},{"type":"text","text":" y"}]}]
W> 8 3:5:t 3:5:t | 2:7:t 2:7:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x foo b baz y"}]}]
L> 8 3:5:t 3:5:t | 2:7:t 2:7:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x foo b y"}]}]
C< 8 3:11:t 3:11:t | 3:7:t 3:7:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x "},{"type":"text","mode":"segmented","text":"foo bar"},{"type":"text","text":" y"}]}]
W< 8 3:11:t 3:11:t | 2:10:t 2:10:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x foo bar  y"}]}]
L< 8 3:11:t 3:11:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x  y"}]}]
C< 8 4:0:t 4:0:t | 4:0:t 4:0:t 0 | =
W< 8 4:0:t 4:0:t | 3:7:t 3:7:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x "},{"type":"text","mode":"segmented","text":"foo bar"},{"type":"text","text":" y"}]}]
L< 8 4:0:t 4:0:t | 3:7:t 3:7:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x "},{"type":"text","mode":"segmented","text":"foo bar"},{"type":"text","text":" y"}]}]
C< 9 3:3:t 3:3:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x  y"}]}]
W< 9 3:3:t 3:3:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x  y"}]}]
L< 9 3:3:t 3:3:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x  y"}]}]
C> 9 3:1:t 3:1:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x  y"}]}]
W> 9 3:1:t 3:1:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x  y"}]}]
L> 9 3:1:t 3:1:t | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"x  y"}]}]
C< 10 3:0:e 3:0:e | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]
W< 10 3:0:e 3:0:e | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]
L< 10 3:0:e 3:0:e | 2:2:t 2:2:t 0 | [{"type":"paragraph","children":[{"type":"text","text":"ab"}]}]
C> 10 3:0:e 3:0:e | 3:0:e 3:0:e 0 | =
W> 10 3:0:e 3:0:e | 3:0:e 3:0:e 0 | =
L> 10 3:0:e 3:0:e | 3:0:e 3:0:e 0 | =
move<c 10 3:0:e 3:0:e | 2:2:t 2:2:t 0 | =
move<w 10 3:0:e 3:0:e | 2:2:t 2:2:t 0 | =
move<l 10 3:0:e 3:0:e | 2:2:t 2:2:t 0 | =
move>c 10 3:0:e 3:0:e | 3:0:e 3:0:e 0 | =
move>w 10 3:0:e 3:0:e | 3:0:e 3:0:e 0 | =
move>l 10 3:0:e 3:0:e | 3:0:e 3:0:e 0 | =
extend<c 10 3:0:e 3:0:e | 3:0:e 2:2:t 0 | =
extend<w 10 3:0:e 3:0:e | 3:0:e 2:2:t 0 | =
extend<l 10 3:0:e 3:0:e | 3:0:e 2:2:t 0 | =
extend>c 10 3:0:e 3:0:e | 3:0:e 3:0:e 0 | =
extend>w 10 3:0:e 3:0:e | 3:0:e 3:0:e 0 | =
extend>l 10 3:0:e 3:0:e | 3:0:e 3:0:e 0 | =
"#;
        let discrete = || UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        };
        for line in cases.lines().filter(|l| !l.is_empty()) {
            let mut parts = line.splitn(3, " | ");
            let given: Vec<&str> = parts.next().unwrap().split(' ').collect();
            let want: Vec<&str> = parts.next().unwrap().split(' ').collect();
            let want_children = parts.next().unwrap();
            let (op, doc, anchor, focus) = (
                given[0].to_owned(),
                given[1].parse::<usize>().unwrap(),
                parse_point(given[2]),
                parse_point(given[3]),
            );
            reset_random_key();
            let editor = create_editor(CreateEditorArgs::default());
            editor.set_selection_movement(Some(Rc::new(TestMovement)));
            let children = json::parse(docs[doc]).unwrap();
            editor
                .update(
                    move || {
                        let nodes: Vec<NodeRef> = children
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|n| parse_serialized_node(n).unwrap())
                            .collect();
                        get_root().append(&nodes)
                    },
                    discrete(),
                )
                .unwrap();
            let before = json::stringify(&editor.get_editor_state().to_json(true)).unwrap();
            editor
                .update(
                    move || {
                        let selection = create_range_selection();
                        selection.point_set(
                            Which::Anchor,
                            &anchor.key,
                            anchor.offset,
                            anchor.point_type,
                        )?;
                        selection.point_set(
                            Which::Focus,
                            &focus.key,
                            focus.offset,
                            focus.point_type,
                        )?;
                        set_selection(Some(selection.clone()));
                        let is_backward = op.contains('<');
                        let granularity = match op.chars().last() {
                            Some('w') => SelectionGranularity::Word,
                            Some('l') => SelectionGranularity::LineBoundary,
                            _ => SelectionGranularity::Character,
                        };
                        match &op[..1] {
                            "C" => selection.delete_character(is_backward),
                            "W" => selection.delete_word(is_backward),
                            "L" => selection.delete_line(is_backward),
                            "m" => selection.modify(SelectionAlter::Move, is_backward, granularity),
                            _ => selection.modify(SelectionAlter::Extend, is_backward, granularity),
                        }
                    },
                    discrete(),
                )
                .unwrap_or_else(|e| panic!("{line}: {e}"));
            let state = editor.get_editor_state();
            let compact = json::stringify(&state.to_json(true)).unwrap();
            let want_compact = if want_children == "=" {
                before
            } else {
                format!(r#"{{"root":{{"type":"root","children":{want_children}}}}}"#)
            };
            assert_eq!(compact, want_compact, "{line}");
            let Some(Selection::Range(range)) = state.selection().map(|s| s.get()) else {
                panic!("{line}: no range selection");
            };
            assert_eq!(
                [
                    show_point(&range.anchor),
                    show_point(&range.focus),
                    range.format.to_string()
                ],
                [want[0], want[1], want[2]],
                "{line}"
            );
        }
    }

    /// Without a movement, `modify` inside text fails as it does in JS on an
    /// editor with no root element.
    #[test]
    fn headless_modify_matches_js() {
        reset_random_key();
        let editor = create_editor(CreateEditorArgs::default());
        let result = editor.update(
            || {
                let paragraph = create_paragraph_node();
                let text = crate::create_text_node("hello big world");
                paragraph.append(std::slice::from_ref(&text))?;
                get_root().append(&[paragraph])?;
                let selection = text.select(Some(7), Some(7))?;
                selection.modify(SelectionAlter::Move, true, SelectionGranularity::Character)
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        );
        assert_eq!(
            result.map_err(|e| e.message().to_owned()),
            Err("window object not found".to_owned())
        );
    }
}
