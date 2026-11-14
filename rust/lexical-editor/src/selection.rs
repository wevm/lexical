//! `@lexical/selection` (the `selection` feature): what of it the ported
//! packages use so far.
//!
//! From `packages/lexical-selection/src/range-selection.ts`.

use crate::caret::{CaretDirection, RootMode};
use crate::caret_utils::{caret_from_point, is_extendable_text_point_caret};
use crate::{LexicalError, NodeRef, Point};

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
