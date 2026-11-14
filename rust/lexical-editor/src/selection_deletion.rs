//! `deleteCharacter`, `deleteWord` and `deleteLine` from
//! `LexicalSelection.ts`, with the parts of `modify` they need headless.
//!
//! JS measures how far a deletion reaches inside text with the DOM's
//! `Selection.modify` (`$extendSelectionForDeletion`), after
//! `$modifySelectionAroundDecoratorsAndBlocks` has resolved the moves that
//! cross blocks in the model. That measurement starts with
//! `getWindow(editor)`, which throws `window object not found` for an editor
//! without a root element, so a headless JS editor fails the update whenever
//! a deletion needs it: within text, at a text node edge with a sibling in
//! the deletion direction, and at the edges of the document. Only the model
//! paths (merging blocks, removing a line break or an empty block, removing
//! an expanded selection) succeed, and that is what this port does.
//!
//! Decorators, slots and shadow roots other than the root are not ported,
//! so the branches for them are no-ops (each is noted where JS has it).

use crate::LexicalError;
use crate::caret::{
    CaretDirection, NodeCaret, PointCaret, RootMode, compare_point_caret_next,
    extend_caret_to_range, get_caret_in_direction, get_caret_range, get_child_caret,
    get_sibling_caret,
};
use crate::caret_utils::{
    caret_from_point, caret_range_from_selection, get_caret_range_in_direction,
    is_extendable_text_point_caret, normalize_caret, rewind_sibling_caret, set_point_from_caret,
    update_range_selection_from_caret_range,
};
use crate::error::invariant;
use crate::node::NodeRef;
use crate::nodes::element::is_root_or_shadow_root;
use crate::nodes::line_break::LineBreakNode;
use crate::nodes::paragraph::{ParagraphNode, create_paragraph_node};
use crate::selection::{Point, PointType, RangeSelection, SelectionRef, Which};
use crate::utf16;
use crate::utils::{get_root, is_block, is_shadow_root_node, restore_empty_container_paragraph};

/// The `granularity` of a deletion: `'character'`, `'word'` or
/// `'lineboundary'`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Granularity {
    Character,
    Word,
    LineBoundary,
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
                        // `$isDecoratorNode(caret.origin)`: decorators are
                        // not ported.
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
            extend_selection_for_deletion(self, is_backward, Granularity::Character)?;

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
        delete_text_by_granularity(self, is_backward, Granularity::Word)
    }

    /// `selection.deleteLine(isBackward)`.
    pub fn delete_line(&self, is_backward: bool) -> Result<(), LexicalError> {
        delete_text_by_granularity(self, is_backward, Granularity::LineBoundary)
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
    granularity: Granularity,
) -> Result<(), LexicalError> {
    let Some(range) = selection.range() else {
        return Ok(());
    };
    let was_collapsed = range.is_collapsed();
    // Set when the deletion stops at a slot boundary, which it never crosses.
    let mut stopped_at_slot = false;
    if was_collapsed {
        if granularity == Granularity::Word
            && forward_deletion(&range.anchor, &range.anchor.get_node()?, is_backward)
        {
            return Ok(());
        }
        stopped_at_slot = extend_selection_for_deletion(selection, is_backward, granularity)?;
        if granularity == Granularity::LineBoundary {
            stop_line_deletion_at_line_break(selection)?;
        }
    }
    // Line deletion must remain in one block; word deletion may cross blocks.
    let range = selection.range().expect("a range selection");
    if granularity == Granularity::LineBoundary
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

/// `$extendSelectionForDeletion(selection, isBackward, granularity)`:
/// extends the collapsed selection over what the deletion removes. Returns
/// true when the focus stopped at a slot boundary (never, as slots are not
/// ported).
///
/// The moves across blocks are resolved in the model. Every other move is
/// measured with the DOM selection of `getWindow(editor)`, which throws for
/// an editor without a root element (`_window` is only set by
/// `setRootElement`), so this returns that error, as headless JS does.
fn extend_selection_for_deletion(
    selection: &SelectionRef,
    is_backward: bool,
    granularity: Granularity,
) -> Result<bool, LexicalError> {
    // Decorator/block handling, resolved in the model exactly as modify()
    // does.
    if modify_selection_around_decorators_and_blocks(selection, is_backward, granularity)? {
        return Ok(false);
    }
    Err(LexicalError::new("window object not found"))
}

/// `$modifySelectionAroundDecoratorsAndBlocks(selection, 'extend',
/// isBackward, granularity)`: moves the focus to the adjacent block when
/// the caret is at the edge of its own and returns true; false when the
/// native measurement has to resolve the move.
///
/// Only the `'extend'` alteration in `'decorators-and-blocks'` mode is
/// ported, the one deletion uses; the `'move'` branches belong to `modify`.
/// Decorators are not ported, so the steps over them are no-ops.
fn modify_selection_around_decorators_and_blocks(
    selection: &SelectionRef,
    is_backward: bool,
    granularity: Granularity,
) -> Result<bool, LexicalError> {
    let range = selection.range().expect("a range selection");
    let initial_focus = caret_from_point(&range.focus, direction(is_backward))?;
    let is_line_boundary = granularity == Granularity::LineBoundary;
    // `focus === initialFocus` in JS: whether a new caret was chosen.
    let mut focus: Option<PointCaret> = None;
    let mut check_for_block = true;
    if !is_extendable_text_point_caret(&initial_focus) {
        if initial_focus.iter().next().is_some() {
            // The first sibling in the deletion direction ends the scan; JS
            // steps over it (or stops at it when isolated) only when it is a
            // decorator.
            check_for_block = false;
        }
        if check_for_block {
            for next_caret in
                extend_caret_to_range(&initial_focus)?.iter_node_carets(RootMode::ShadowRoot)
            {
                if next_caret.is_child_caret() {
                    if !next_caret.origin().is_inline() {
                        focus = Some(next_caret);
                    }
                } else if next_caret.origin().is_element() {
                    continue;
                }
                // A block decorator would be the focus too; decorators are
                // not ported.
                break;
            }
        }
    }
    let Some(focus) = focus else {
        return Ok(false);
    };
    // After this point checkForBlock is true if and only if we moved to a
    // different block, so we should stop regardless of the granularity.
    set_point_from_caret(selection, Which::Focus, &normalize_caret(&focus))?;
    Ok(check_for_block || !is_line_boundary)
}

/// `$needsBlockCursorBeside(node)`: whether a caret beside the node needs a
/// block cursor, as a top-level shadow root and an element that can't be
/// empty do (and a block decorator, once decorators are ported).
fn needs_block_cursor_beside(node: &NodeRef) -> bool {
    if node.is_inline() {
        return false;
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
            if element_collapse_at_start(&n, selection)? {
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

/// `node.collapseAtStart(selection)` of the ported elements: `RootNode`
/// returns true, `ParagraphNode` removes itself when it holds only blank
/// text, and `ElementNode` returns false.
fn element_collapse_at_start(
    node: &NodeRef,
    _selection: &SelectionRef,
) -> Result<bool, LexicalError> {
    if node.is_root() {
        return Ok(true);
    }
    if !node.get_latest().is::<ParagraphNode>() {
        return Ok(false);
    }
    // `ParagraphNode.collapseAtStart()`: an empty (trimmed) paragraph is
    // removed as long as there is a sibling to go to. Every child has to be
    // blank text.
    if node
        .get_children()
        .iter()
        .all(|child| child.is_text() && child.get_text_content().chars().all(is_js_whitespace))
    {
        if node.get_next_sibling().is_some() {
            node.select_next(None, None)?;
            node.remove(false)?;
            return Ok(true);
        }
        if node.get_previous_sibling().is_some() {
            node.select_previous(None, None)?;
            node.remove(false)?;
            return Ok(true);
        }
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
fn is_js_whitespace(c: char) -> bool {
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
    use crate::selection::{Selection, create_range_selection, set_selection};
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
}
