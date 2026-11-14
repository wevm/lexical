//! `caret/LexicalCaret.ts`: carets, caret ranges, text slices and the
//! document order of carets.
//!
//! JS has one class per caret type and direction (`SiblingCaretNext`,
//! `ChildCaretLast`, `TextPointCaretPrevious`, ...). Here one [`PointCaret`]
//! value carries its [`CaretKind`], the JS `type` (with the offset of a text
//! point caret), and its [`CaretDirection`]; [`SiblingCaret`],
//! [`ChildCaret`], [`TextPointCaret`] and [`NodeCaret`] are aliases of it
//! that say which kinds a function takes or returns, as the JS types do.
//!
//! A caret's origin is a [`NodeRef`], which always resolves the latest
//! version of the node, so `getLatest()` returns the same caret. Slots are
//! not ported, so no node is a slot value and `$filterByMode` only stops at
//! roots.

use crate::LexicalError;
use crate::error::invariant;
use crate::lexical_utils::get_root;
use crate::node::{NodeKey, NodeRef};
use crate::nodes::element::is_root_or_shadow_root;
use crate::utf16;
use std::cmp::Ordering;
use std::collections::HashMap;

/// `CaretDirection`: `'next'` points towards the end of the document,
/// `'previous'` towards the beginning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CaretDirection {
    Next,
    Previous,
}

impl CaretDirection {
    /// The JS name: `"next"` or `"previous"`.
    pub fn as_str(self) -> &'static str {
        match self {
            CaretDirection::Next => "next",
            CaretDirection::Previous => "previous",
        }
    }
}

/// `flipDirection(direction)`.
pub fn flip_direction(direction: CaretDirection) -> CaretDirection {
    match direction {
        CaretDirection::Next => CaretDirection::Previous,
        CaretDirection::Previous => CaretDirection::Next,
    }
}

/// `RootMode`: whether a traversal up the tree stops at the root only
/// (`'root'`), or at the root and any shadow root (`'shadowRoot'`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RootMode {
    #[default]
    Root,
    ShadowRoot,
}

/// A caret's JS `type`, with the offset a text point caret carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CaretKind {
    /// `'sibling'`: from the origin towards its next or previous sibling.
    Sibling,
    /// `'child'`: from an element origin towards its first (`next`) or
    /// last (`previous`) child.
    Child,
    /// `'text'`: at `offset` (UTF-16 code units) in a text node origin.
    Text { offset: usize },
}

/// `PointCaret`: a [`NodeCaret`] or a [`TextPointCaret`].
///
/// Two carets are `==` when JS's `isSamePointCaret` holds: same kind,
/// direction, origin (by key) and offset.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PointCaret {
    origin: NodeRef,
    direction: CaretDirection,
    kind: CaretKind,
}

/// `SiblingCaret`: a [`PointCaret`] whose kind is [`CaretKind::Sibling`].
pub type SiblingCaret = PointCaret;
/// `ChildCaret`: a [`PointCaret`] whose kind is [`CaretKind::Child`].
pub type ChildCaret = PointCaret;
/// `TextPointCaret`: a [`PointCaret`] whose kind is [`CaretKind::Text`].
pub type TextPointCaret = PointCaret;
/// `NodeCaret`: a sibling or child caret.
pub type NodeCaret = PointCaret;

/// `$filterByMode(node, mode)`: `None` at a root according to `mode`.
fn filter_by_mode(node: Option<NodeRef>, mode: RootMode) -> Option<NodeRef> {
    let node = node?;
    let is_boundary = match mode {
        RootMode::Root => node.is_root(),
        RootMode::ShadowRoot => is_root_or_shadow_root(&*node.get_latest()),
    };
    // A slot value is a boundary too, once slots are ported.
    (!is_boundary).then_some(node)
}

impl PointCaret {
    /// `caret.origin`.
    pub fn origin(&self) -> &NodeRef {
        &self.origin
    }

    /// `caret.direction`.
    pub fn direction(&self) -> CaretDirection {
        self.direction
    }

    pub fn kind(&self) -> CaretKind {
        self.kind
    }

    /// `caret.type`: `"sibling"`, `"child"` or `"text"`.
    pub fn caret_type(&self) -> &'static str {
        match self.kind {
            CaretKind::Sibling => "sibling",
            CaretKind::Child => "child",
            CaretKind::Text { .. } => "text",
        }
    }

    /// `caret.offset` of a text point caret.
    pub fn offset(&self) -> Option<usize> {
        match self.kind {
            CaretKind::Text { offset } => Some(offset),
            _ => None,
        }
    }

    /// `$isTextPointCaret(caret)`.
    pub fn is_text_point_caret(&self) -> bool {
        matches!(self.kind, CaretKind::Text { .. })
    }

    /// `$isSiblingCaret(caret)`: a sibling caret, not a text point caret.
    pub fn is_sibling_caret(&self) -> bool {
        self.kind == CaretKind::Sibling
    }

    /// `$isChildCaret(caret)`.
    pub fn is_child_caret(&self) -> bool {
        self.kind == CaretKind::Child
    }

    /// `getLatest()`: the origin always resolves its latest version, so
    /// this is the same caret.
    pub fn get_latest(&self) -> PointCaret {
        self.clone()
    }

    /// `getNodeAtCaret()`: the node next to the origin in the caret's
    /// direction (the first or last child for a child caret).
    pub fn get_node_at_caret(&self) -> Option<NodeRef> {
        match (self.kind, self.direction) {
            (CaretKind::Child, CaretDirection::Next) => self.origin.get_first_child(),
            (CaretKind::Child, CaretDirection::Previous) => self.origin.get_last_child(),
            (_, CaretDirection::Next) => self.origin.get_next_sibling(),
            (_, CaretDirection::Previous) => self.origin.get_previous_sibling(),
        }
    }

    /// `getParentAtCaret()`: the origin for a child caret, the origin's
    /// parent otherwise.
    pub fn get_parent_at_caret(&self) -> Option<NodeRef> {
        match self.kind {
            CaretKind::Child => Some(self.origin.clone()),
            _ => self.origin.get_parent(),
        }
    }

    /// `getAdjacentCaret()`: the sibling caret of the node at the caret.
    pub fn get_adjacent_caret(&self) -> Option<SiblingCaret> {
        self.get_node_at_caret()
            .map(|node| get_sibling_caret(&node, self.direction))
    }

    /// `getSiblingCaret()`: a sibling caret with the same origin and
    /// direction.
    pub fn get_sibling_caret(&self) -> SiblingCaret {
        match self.kind {
            CaretKind::Sibling => self.clone(),
            _ => get_sibling_caret(&self.origin, self.direction),
        }
    }

    /// `getChildCaret()`: this caret for a child caret, the child caret of
    /// an element origin for a sibling caret, `None` otherwise.
    pub fn get_child_caret(&self) -> Option<ChildCaret> {
        match self.kind {
            CaretKind::Child => Some(self.clone()),
            CaretKind::Sibling => get_child_caret(&self.origin, self.direction),
            CaretKind::Text { .. } => None,
        }
    }

    /// `getParentCaret(mode)`: the sibling caret of the parent at the caret
    /// in the same direction, `None` at a root according to `mode`.
    pub fn get_parent_caret(&self, mode: RootMode) -> Option<SiblingCaret> {
        filter_by_mode(self.get_parent_at_caret(), mode)
            .map(|parent| get_sibling_caret(&parent, self.direction))
    }

    /// `getFlipped()`: the same position facing the other way, so that
    /// flipping twice is the identity.
    pub fn get_flipped(&self) -> Result<PointCaret, LexicalError> {
        let dir = flip_direction(self.direction);
        Ok(match self.kind {
            CaretKind::Text { offset } => PointCaret {
                origin: self.origin.clone(),
                direction: dir,
                kind: CaretKind::Text { offset },
            },
            CaretKind::Child => match self.get_node_at_caret() {
                Some(node) => get_sibling_caret(&node, dir),
                None => new_child_caret(&self.origin, dir),
            },
            CaretKind::Sibling => match self.get_node_at_caret() {
                Some(node) => get_sibling_caret(&node, dir),
                None => new_child_caret(&self.origin.get_parent_or_throw()?, dir),
            },
        })
    }

    /// `isSameNodeCaret(other)`: same direction and origin, where a sibling
    /// caret and a text point caret count as the same.
    pub fn is_same_node_caret(&self, other: &PointCaret) -> bool {
        let same_kind = match self.kind {
            CaretKind::Child => other.is_child_caret(),
            _ => !other.is_child_caret(),
        };
        same_kind && self.direction == other.direction && self.origin == other.origin
    }

    /// `isSamePointCaret(other)`: the same caret (also `==`).
    pub fn is_same_point_caret(&self, other: &PointCaret) -> bool {
        self == other
    }

    /// `[Symbol.iterator]()`: the sibling carets of the nodes after the
    /// caret, in its direction.
    pub fn iter(&self) -> impl Iterator<Item = SiblingCaret> + use<> {
        make_stepwise_iterator(
            self.get_adjacent_caret(),
            |caret: &Option<SiblingCaret>| caret.as_ref().is_some_and(PointCaret::is_sibling_caret),
            |caret| caret.and_then(|c| c.get_adjacent_caret()),
            |caret| caret.clone().expect("checked by has_next"),
        )
    }

    /// `remove()`: removes the node at the caret, if any.
    pub fn remove(&self) -> Result<(), LexicalError> {
        if let Some(node) = self.get_node_at_caret() {
            node.remove(false)?;
        }
        Ok(())
    }

    /// `insert(node)`: inserts `node` next to the origin in the caret's
    /// direction (before the node at the caret).
    pub fn insert(&self, node: &NodeRef) -> Result<(), LexicalError> {
        match (self.kind, self.direction) {
            (CaretKind::Child, CaretDirection::Next) => {
                self.origin.splice(0, 0, std::slice::from_ref(node))
            }
            (CaretKind::Child, CaretDirection::Previous) => self.origin.splice(
                self.origin.get_children_size(),
                0,
                std::slice::from_ref(node),
            ),
            (_, CaretDirection::Next) => self.origin.insert_after(node, true).map(drop),
            (_, CaretDirection::Previous) => self.origin.insert_before(node, true).map(drop),
        }
    }

    /// `replaceOrInsert(node, includeChildren)`: replaces the node at the
    /// caret with `node`, or inserts it if there is none.
    pub fn replace_or_insert(
        &self,
        node: &NodeRef,
        include_children: bool,
    ) -> Result<(), LexicalError> {
        let target = self.get_node_at_caret();
        if node == &self.origin || Some(node) == target.as_ref() {
            // do nothing
        } else if let Some(target) = target {
            target.replace(node, include_children)?;
        } else {
            self.insert(node)?;
        }
        Ok(())
    }

    /// `splice(deleteCount, nodes, nodesDirection)`: replaces the
    /// `delete_count` nodes after the caret with `nodes`, which are in
    /// `nodes_direction` order (JS's default is `next`).
    pub fn splice(
        &self,
        delete_count: usize,
        nodes: &[NodeRef],
        nodes_direction: CaretDirection,
    ) -> Result<(), LexicalError> {
        let mut node_iter = nodes.to_vec();
        if nodes_direction != self.direction {
            node_iter.reverse();
        }
        let mut caret = self.clone();
        let parent = self.get_parent_at_caret();
        // Insertion-ordered, like JS's Map.
        let mut nodes_to_remove: Vec<NodeRef> = Vec::new();
        // Find all of the nodes we expect to remove first, so we don't have
        // to worry about the cases where there is overlap between the nodes
        // to insert and the nodes to remove
        let mut remove_caret = caret.get_adjacent_caret();
        while let Some(rc) = remove_caret.filter(|_| nodes_to_remove.len() < delete_count) {
            crate::lexical_utils::get_writable(rc.origin.get_key());
            if !nodes_to_remove.contains(&rc.origin) {
                nodes_to_remove.push(rc.origin.clone());
            }
            remove_caret = rc.get_adjacent_caret();
        }
        for node in &node_iter {
            if !nodes_to_remove.is_empty() {
                let Some(target) = caret.get_node_at_caret() else {
                    let keys: Vec<&str> = nodes_to_remove.iter().map(NodeRef::get_key).collect();
                    return Err(LexicalError::new(format!(
                        "NodeCaret.splice: Underflow of expected nodesToRemove during splice (keys: {})",
                        keys.join(" ")
                    )));
                };
                nodes_to_remove.retain(|n| n != &target && n != node);
                if &target == node || &caret.origin == node {
                    // do nothing, it's already in the right place
                } else {
                    if node
                        .get_parent()
                        .is_some_and(|p| Some(&p) == parent.as_ref())
                    {
                        // It's a sibling somewhere else in this node, so
                        // unparent it first
                        node.remove(false)?;
                    }
                    target.replace(node, false)?;
                }
            } else {
                caret.insert(node)?;
            }
            caret = get_sibling_caret(node, self.direction);
        }
        for node in &nodes_to_remove {
            node.remove(false)?;
        }
        Ok(())
    }
}

fn new_child_caret(origin: &NodeRef, direction: CaretDirection) -> ChildCaret {
    PointCaret {
        origin: origin.clone(),
        direction,
        kind: CaretKind::Child,
    }
}

/// `$getSiblingCaret(origin, direction)`: a caret pointing from `origin` to
/// its next or previous sibling. JS returns `null` for a `null` origin;
/// here that is `Option::map`.
pub fn get_sibling_caret(origin: &NodeRef, direction: CaretDirection) -> SiblingCaret {
    PointCaret {
        origin: origin.clone(),
        direction,
        kind: CaretKind::Sibling,
    }
}

/// An offset into a text node: a number of UTF-16 code units, or the end
/// in a direction (`'next'` is the end, `'previous'` the start), as JS's
/// `number | CaretDirection`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOffset {
    Offset(usize),
    End(CaretDirection),
}

impl From<usize> for TextOffset {
    fn from(offset: usize) -> Self {
        TextOffset::Offset(offset)
    }
}

impl From<CaretDirection> for TextOffset {
    fn from(direction: CaretDirection) -> Self {
        TextOffset::End(direction)
    }
}

/// `$getTextPointCaret(origin, direction, offset)`: a caret at `offset` in
/// the text node `origin`. An offset past the end is an error, as in JS's
/// development build.
pub fn get_text_point_caret(
    origin: &NodeRef,
    direction: CaretDirection,
    offset: impl Into<TextOffset>,
) -> Result<TextPointCaret, LexicalError> {
    Ok(PointCaret {
        origin: origin.clone(),
        direction,
        kind: CaretKind::Text {
            offset: get_text_node_offset(origin, offset.into(), false)?,
        },
    })
}

/// [`get_text_point_caret`] at an end of the text, which can't fail.
pub(crate) fn get_text_point_caret_at_end(
    origin: &NodeRef,
    direction: CaretDirection,
    end: CaretDirection,
) -> TextPointCaret {
    get_text_point_caret(origin, direction, end).expect("an end is always a valid offset")
}

/// `$getTextNodeOffset(origin, offset, mode)`: the numeric offset of
/// `offset` in `origin`. Out of bounds it is an error (JS's development
/// build), or clamped when `clamp` (`mode` `'clamp'`).
pub fn get_text_node_offset(
    origin: &NodeRef,
    offset: TextOffset,
    clamp: bool,
) -> Result<usize, LexicalError> {
    let size = origin.get_text_content_size();
    let numeric_offset = match offset {
        TextOffset::End(CaretDirection::Next) => size,
        TextOffset::End(CaretDirection::Previous) => 0,
        TextOffset::Offset(offset) => offset,
    };
    if numeric_offset > size {
        invariant!(
            clamp,
            "$getTextNodeOffset: invalid offset {numeric_offset} for size {size} at key {}",
            origin.get_key()
        );
        return Ok(size);
    }
    Ok(numeric_offset)
}

/// `TextPointCaretSlice`: a text point caret and a signed distance from its
/// offset. A negative distance selects the text before the offset, a
/// positive one the text after it, whatever the caret's direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextPointCaretSlice {
    pub caret: TextPointCaret,
    pub distance: isize,
}

impl TextPointCaretSlice {
    fn offset(&self) -> usize {
        self.caret.offset().expect("a text point caret")
    }

    /// `getSliceIndices()`: the start and end of the slice in the text.
    pub fn get_slice_indices(&self) -> (usize, usize) {
        let offset = self.offset();
        let offset_b = offset.saturating_add_signed(self.distance);
        if offset_b < offset {
            (offset_b, offset)
        } else {
            (offset, offset_b)
        }
    }

    /// `getTextContent()`: the text of the slice.
    pub fn get_text_content(&self) -> String {
        let (start, end) = self.get_slice_indices();
        utf16::slice(&self.caret.origin.get_text_content(), start, end)
    }

    /// `getTextContentSize()`.
    pub fn get_text_content_size(&self) -> usize {
        self.distance.unsigned_abs()
    }

    /// `removeTextSlice()`: removes the slice's text, and returns the caret
    /// at its start, in the same direction.
    pub fn remove_text_slice(&self) -> Result<TextPointCaret, LexicalError> {
        let origin = &self.caret.origin;
        let (start, end) = self.get_slice_indices();
        // `$setTextContentSpliced`: text edits aren't recorded in this port.
        let content = origin.get_text_content();
        let next = utf16::slice(&content, 0, start) + &utf16::slice_from(&content, end);
        origin.set_text_content(&next);
        get_text_point_caret(origin, self.caret.direction, start)
    }
}

/// `$getTextPointCaretSlice(caret, distance)`.
pub fn get_text_point_caret_slice(caret: TextPointCaret, distance: isize) -> TextPointCaretSlice {
    debug_assert!(caret.is_text_point_caret());
    TextPointCaretSlice { caret, distance }
}

/// `$getChildCaret(origin, direction)`: a caret pointing to the first
/// (`next`) or last (`previous`) child of `origin`, or `None` if it isn't
/// an element.
pub fn get_child_caret(origin: &NodeRef, direction: CaretDirection) -> Option<ChildCaret> {
    origin
        .is_element()
        .then(|| new_child_caret(origin, direction))
}

/// `$getChildCaretOrSelf(caret)`: the caret's child caret if it has one,
/// otherwise the caret.
pub fn get_child_caret_or_self(caret: Option<PointCaret>) -> Option<PointCaret> {
    caret.map(|c| c.get_child_caret().unwrap_or(c))
}

/// `$getAdjacentChildCaret(caret)`: the adjacent caret, or its child caret
/// when its origin is an element. With `getParentCaret` this walks the
/// whole tree depth first.
pub fn get_adjacent_child_caret(caret: Option<&NodeCaret>) -> Option<NodeCaret> {
    get_child_caret_or_self(caret?.get_adjacent_caret())
}

/// `CaretRange`: a range selection as a pair of carets in one direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaretRange {
    pub anchor: PointCaret,
    pub focus: PointCaret,
    direction: CaretDirection,
}

impl CaretRange {
    /// `range.direction`.
    pub fn direction(&self) -> CaretDirection {
        self.direction
    }

    /// `getLatest()`: the same range, as carets always resolve the latest
    /// nodes.
    pub fn get_latest(&self) -> CaretRange {
        self.clone()
    }

    /// `isCollapsed()`: whether anchor and focus are the same caret.
    pub fn is_collapsed(&self) -> bool {
        self.anchor.is_same_point_caret(&self.focus)
    }

    /// `getTextSlices()`: the slices of the text point carets at the anchor
    /// and the focus. When both are on one text node, the first holds the
    /// whole slice and the second is `None`.
    pub fn get_text_slices(&self) -> [Option<TextPointCaretSlice>; 2] {
        let anchor = self.anchor.get_latest();
        let focus = self.focus.get_latest();
        if let (Some(anchor_offset), Some(focus_offset)) = (anchor.offset(), focus.offset())
            && anchor.is_same_node_caret(&focus)
        {
            let distance = focus_offset as isize - anchor_offset as isize;
            return [Some(get_text_point_caret_slice(anchor, distance)), None];
        }
        [
            anchor
                .is_text_point_caret()
                .then(|| get_slice_from_text_point_caret(anchor, false)),
            focus
                .is_text_point_caret()
                .then(|| get_slice_from_text_point_caret(focus, true)),
        ]
    }

    /// `iterNodeCarets(rootMode)`: the carets between anchor and focus, in
    /// pre-order, without the text slices at either end. An element is
    /// yielded as a child caret on enter and a sibling caret on leave.
    pub fn iter_node_carets(&self, root_mode: RootMode) -> impl Iterator<Item = NodeCaret> + use<> {
        let anchor = if self.anchor.is_text_point_caret() {
            self.anchor.get_sibling_caret()
        } else {
            self.anchor.get_latest()
        };
        let focus = self.focus.get_latest();
        let is_text_focus = focus.is_text_point_caret();
        let step = {
            let focus = focus.clone();
            move |state: &NodeCaret| -> Option<NodeCaret> {
                if state.is_same_node_caret(&focus) {
                    None
                } else {
                    get_adjacent_child_caret(Some(state))
                        .or_else(|| state.get_parent_caret(root_mode))
                }
            }
        };
        let initial = if anchor.is_same_node_caret(&focus) {
            None
        } else {
            step(&anchor)
        };
        make_stepwise_iterator(
            initial,
            move |state: &Option<NodeCaret>| {
                state
                    .as_ref()
                    .is_some_and(|s| !(is_text_focus && focus.is_same_node_caret(s)))
            },
            move |state| state.and_then(|s| step(&s)),
            |state| state.clone().expect("checked by has_next"),
        )
    }

    /// `[Symbol.iterator]()`: `iterNodeCarets('root')`.
    pub fn iter(&self) -> impl Iterator<Item = NodeCaret> + use<> {
        self.iter_node_carets(RootMode::Root)
    }
}

/// `$getSliceFromTextPointCaret(caret, anchorOrFocus)`: the slice from the
/// caret to the end of its text the range covers.
fn get_slice_from_text_point_caret(caret: TextPointCaret, is_focus: bool) -> TextPointCaretSlice {
    let direction = caret.direction;
    let end = if is_focus {
        flip_direction(direction)
    } else {
        direction
    };
    let offset_b = get_text_node_offset(&caret.origin, end.into(), false)
        .expect("an end is always a valid offset");
    let offset = caret.offset().expect("a text point caret");
    get_text_point_caret_slice(caret, offset_b as isize - offset as isize)
}

/// `$getCaretInDirection(caret, direction)`: the caret if it is in
/// `direction`, otherwise `caret.getFlipped()`.
pub fn get_caret_in_direction(
    caret: &PointCaret,
    direction: CaretDirection,
) -> Result<PointCaret, LexicalError> {
    if caret.direction == direction {
        Ok(caret.clone())
    } else {
        caret.get_flipped()
    }
}

/// `$extendCaretToRange(anchor)`: a range from `anchor` to the end of the
/// document in its direction.
pub fn extend_caret_to_range(anchor: &PointCaret) -> Result<CaretRange, LexicalError> {
    let root_caret = new_child_caret(&get_root(), flip_direction(anchor.direction));
    get_caret_range(
        anchor.clone(),
        get_caret_in_direction(&root_caret, anchor.direction)?,
    )
}

/// `$getCollapsedCaretRange(anchor)`.
pub fn get_collapsed_caret_range(anchor: PointCaret) -> CaretRange {
    CaretRange {
        anchor: anchor.clone(),
        focus: anchor.clone(),
        direction: anchor.direction,
    }
}

/// `$getCaretRange(anchor, focus)`: a range between carets in the same
/// direction, where the anchor points towards the focus.
pub fn get_caret_range(anchor: PointCaret, focus: PointCaret) -> Result<CaretRange, LexicalError> {
    invariant!(
        anchor.direction == focus.direction,
        "$getCaretRange: anchor and focus must be in the same direction"
    );
    let direction = anchor.direction;
    Ok(CaretRange {
        anchor,
        focus,
        direction,
    })
}

/// `makeStepwiseIterator({initial, hasNext, step, map})`: yields
/// `map(state)` while `has_next(state)`, stepping the state after each.
pub fn make_stepwise_iterator<State, Value>(
    initial: State,
    has_next: impl FnMut(&State) -> bool,
    step: impl FnMut(State) -> State,
    map: impl FnMut(&State) -> Value,
) -> impl Iterator<Item = Value> {
    StepwiseIterator {
        state: Some(initial),
        has_next,
        step,
        map,
    }
}

struct StepwiseIterator<State, H, S, M> {
    // Only `None` while stepping.
    state: Option<State>,
    has_next: H,
    step: S,
    map: M,
}

impl<State, Value, H, S, M> Iterator for StepwiseIterator<State, H, S, M>
where
    H: FnMut(&State) -> bool,
    S: FnMut(State) -> State,
    M: FnMut(&State) -> Value,
{
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        let state = self.state.as_ref()?;
        if !(self.has_next)(state) {
            return None;
        }
        let value = (self.map)(state);
        let state = self.state.take().expect("checked above");
        self.state = Some((self.step)(state));
        Some(value)
    }
}

/// `$comparePointCaretNext(a, b)`: the total order of `next` carets in
/// which a [`CaretRange`] iterates them. For one origin, a child caret and
/// a text point caret come before the sibling caret. Carets whose origins
/// have no common ancestor are an error.
pub fn compare_point_caret_next(a: &PointCaret, b: &PointCaret) -> Result<Ordering, LexicalError> {
    let compare = get_common_ancestor(&a.origin, &b.origin);
    let Some(compare) = compare else {
        return Err(LexicalError::new(format!(
            "$comparePointCaretNext: a (key {}) and b (key {}) do not have a common ancestor",
            a.origin.get_key(),
            b.origin.get_key()
        )));
    };
    Ok(match compare {
        CommonAncestorResult::Same { .. } => compare_same_origin_carets(a, b),
        CommonAncestorResult::Ancestor { .. } => {
            if a.is_child_caret() {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
        CommonAncestorResult::Descendant { .. } => {
            if b.is_child_caret() {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        }
        CommonAncestorResult::Branch { a, b, .. } => {
            get_common_ancestor_result_branch_order(&a, &b)
        }
    })
}

/// `$compareSameOriginCarets(a, b)`.
fn compare_same_origin_carets(a: &PointCaret, b: &PointCaret) -> Ordering {
    match (a.kind, b.kind) {
        (CaretKind::Text { offset: x }, CaretKind::Text { offset: y }) => x.cmp(&y),
        (x, y) if x == y => Ordering::Equal,
        (CaretKind::Text { .. }, _) => Ordering::Less,
        (_, CaretKind::Text { .. }) => Ordering::Greater,
        (CaretKind::Child, _) => Ordering::Less,
        _ => Ordering::Greater,
    }
}

/// `$getCommonAncestorResultBranchOrder(compare)`: `Less` if `a` (a sibling
/// of `b`) comes first, `Greater` otherwise.
pub fn get_common_ancestor_result_branch_order(a: &NodeRef, b: &NodeRef) -> Ordering {
    let mut na = Some(a.clone());
    let mut nb = Some(b.clone());
    while let (Some(x), Some(y)) = (&na, &nb) {
        if x == b {
            return Ordering::Less;
        } else if y == a {
            return Ordering::Greater;
        }
        na = x.get_next_sibling();
        nb = y.get_next_sibling();
    }
    if na.is_none() {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

/// `CommonAncestorResult`: how two nodes relate through their common
/// ancestor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommonAncestorResult {
    /// The two nodes are the same.
    Same { common_ancestor: NodeRef },
    /// `a` is an ancestor of `b`.
    Ancestor { common_ancestor: NodeRef },
    /// `a` is a descendant of `b`.
    Descendant { common_ancestor: NodeRef },
    /// `a` and `b` are on different branches; `a` and `b` here are their
    /// ancestors (or themselves) that are children of `common_ancestor`.
    Branch {
        common_ancestor: NodeRef,
        a: NodeRef,
        b: NodeRef,
    },
}

impl CommonAncestorResult {
    /// `result.commonAncestor`.
    pub fn common_ancestor(&self) -> &NodeRef {
        match self {
            CommonAncestorResult::Same { common_ancestor }
            | CommonAncestorResult::Ancestor { common_ancestor }
            | CommonAncestorResult::Descendant { common_ancestor }
            | CommonAncestorResult::Branch {
                common_ancestor, ..
            } => common_ancestor,
        }
    }
}

/// `$initialElementTuple(node)`: the node and no child for an element, its
/// parent and itself otherwise.
fn initial_element_tuple(node: &NodeRef) -> (Option<NodeRef>, Option<NodeRef>) {
    if node.is_element() {
        (Some(node.clone()), None)
    } else {
        (node.get_parent(), Some(node.clone()))
    }
}

/// `$getCommonAncestor(a, b)`: how `a` and `b` relate, or `None` if they
/// have no common ancestor.
pub fn get_common_ancestor(a: &NodeRef, b: &NodeRef) -> Option<CommonAncestorResult> {
    if a == b {
        return Some(CommonAncestorResult::Same {
            common_ancestor: a.clone(),
        });
    }
    // Map of parent -> child entries based on a and its ancestors
    let mut a_map: HashMap<NodeKey, Option<NodeRef>> = HashMap::new();
    let (mut parent, mut child) = initial_element_tuple(a);
    while let Some(p) = parent {
        parent = p.get_parent();
        a_map.insert(p.get_key().to_owned(), child);
        child = Some(p);
    }
    let (mut parent, mut child) = initial_element_tuple(b);
    while let Some(p) = parent {
        if let Some(a_child) = a_map.get(p.get_key()) {
            return Some(match (a_child, child) {
                (None, _) => {
                    // a is the ancestor
                    assert!(&p == a, "$originComparison: ancestor logic error");
                    CommonAncestorResult::Ancestor { common_ancestor: p }
                }
                (Some(_), None) => {
                    // b is the ancestor
                    assert!(&p == b, "$originComparison: descendant logic error");
                    CommonAncestorResult::Descendant { common_ancestor: p }
                }
                (Some(a_child), Some(child)) => {
                    assert!(
                        (a_child.is_element() || a_child == a)
                            && (child.is_element() || &child == b)
                            && a_child.get_parent().as_ref() == Some(&p)
                            && child.get_parent().as_ref() == Some(&p),
                        "$originComparison: branch logic error"
                    );
                    CommonAncestorResult::Branch {
                        common_ancestor: p,
                        a: a_child.clone(),
                        b: child,
                    }
                }
            });
        }
        parent = p.get_parent();
        child = Some(p);
    }
    None
}
