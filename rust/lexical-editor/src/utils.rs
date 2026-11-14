//! `@lexical/utils` (the `utils` feature): what of it the ported packages
//! use so far.
//!
//! From `packages/lexical-utils/src/index.ts`.

use crate::caret::CaretDirection;
use crate::selection::is_at_edge_of_element;
use crate::{LexicalError, NodeRef, Point};

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
