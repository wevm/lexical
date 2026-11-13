//! `LexicalSelection.ts`: points, `RangeSelection` and `NodeSelection`, and
//! the helpers node methods use to keep a selection valid while they change
//! the tree. Selection editing (`insertText`, `deleteCharacter`, ...) comes
//! next (rust/DESIGN.md, M1).

use crate::LexicalError;
use crate::error::invariant;
use crate::node::{NodeKey, NodeRef};
use crate::ordered::OrderedSet;
use crate::updates::{get_active_editor_state, is_currently_read_only_mode};
use crate::utils::get_node_by_key;
use std::cell::RefCell;
use std::rc::Rc;

/// A point's type: inside a text node (an offset in UTF-16 code units) or
/// between an element's children (a child index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointType {
    Text,
    Element,
}

impl PointType {
    pub fn as_str(self) -> &'static str {
        match self {
            PointType::Text => "text",
            PointType::Element => "element",
        }
    }
}

/// `PointType`: a node key, an offset and a type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub key: NodeKey,
    pub offset: usize,
    pub point_type: PointType,
}

impl Point {
    pub fn new(key: impl Into<NodeKey>, offset: usize, point_type: PointType) -> Self {
        Self {
            key: key.into(),
            offset,
            point_type,
        }
    }

    /// `point.getNode()`.
    pub fn get_node(&self) -> Result<NodeRef, LexicalError> {
        get_node_by_key(&self.key).ok_or_else(|| LexicalError::new("Point.getNode: node not found"))
    }
}

/// Which point of a range selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Anchor,
    Focus,
}

/// `RangeSelection`'s fields.
#[derive(Clone, Debug)]
pub struct RangeSelection {
    pub anchor: Point,
    pub focus: Point,
    pub format: f64,
    pub style: String,
    pub(crate) dirty: bool,
    pub(crate) cached_is_backward: Option<bool>,
}

impl RangeSelection {
    /// `new RangeSelection(anchor, focus, format, style)`.
    pub fn new(anchor: Point, focus: Point, format: f64, style: impl Into<String>) -> Self {
        Self {
            anchor,
            focus,
            format,
            style: style.into(),
            dirty: false,
            cached_is_backward: None,
        }
    }

    pub fn point(&self, which: Which) -> &Point {
        match which {
            Which::Anchor => &self.anchor,
            Which::Focus => &self.focus,
        }
    }

    fn point_mut(&mut self, which: Which) -> &mut Point {
        match which {
            Which::Anchor => &mut self.anchor,
            Which::Focus => &mut self.focus,
        }
    }

    /// `is(selection)`: equal points, format and style.
    pub fn is(&self, other: &RangeSelection) -> bool {
        self.anchor == other.anchor
            && self.focus == other.focus
            && self.format == other.format
            && self.style == other.style
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }
}

/// `NodeSelection`'s fields: the selected keys, in the order added.
#[derive(Clone, Debug, Default)]
pub struct NodeSelection {
    pub(crate) nodes: OrderedSet,
    pub(crate) dirty: bool,
}

impl NodeSelection {
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter()
    }

    pub fn add(&mut self, key: &str) {
        self.dirty = true;
        self.nodes.add(key);
    }

    pub fn delete(&mut self, key: &str) {
        self.dirty = true;
        self.nodes.delete(key);
    }

    pub fn has(&self, key: &str) -> bool {
        self.nodes.contains(key)
    }
}

/// A selection's value.
#[derive(Clone, Debug)]
pub enum Selection {
    Range(RangeSelection),
    Node(NodeSelection),
}

impl Selection {
    fn dirty_mut(&mut self) -> &mut bool {
        match self {
            Selection::Range(s) => &mut s.dirty,
            Selection::Node(s) => &mut s.dirty,
        }
    }

    /// `selection.is(other)`.
    pub fn is(&self, other: &Selection) -> bool {
        match (self, other) {
            (Selection::Range(a), Selection::Range(b)) => a.is(b),
            (Selection::Node(a), Selection::Node(b)) => {
                a.nodes.len() == b.nodes.len() && a.nodes.iter().all(|k| b.nodes.contains(k))
            }
            _ => false,
        }
    }
}

/// A selection object, shared as JS shares it: `$getSelection()` returns the
/// one the editor state holds, so a change through it is a change of the
/// state's selection; [`clone_selection`](Self::clone_selection) makes a
/// separate one.
#[derive(Clone, Debug)]
pub struct SelectionRef(Rc<RefCell<Selection>>);

impl SelectionRef {
    pub fn new(selection: Selection) -> Self {
        Self(Rc::new(RefCell::new(selection)))
    }

    /// The selection's current value.
    pub fn get(&self) -> Selection {
        self.0.borrow().clone()
    }

    pub fn is_range(&self) -> bool {
        matches!(*self.0.borrow(), Selection::Range(_))
    }

    pub fn is_node(&self) -> bool {
        matches!(*self.0.borrow(), Selection::Node(_))
    }

    /// The range selection's value, `None` for a node selection.
    pub fn range(&self) -> Option<RangeSelection> {
        match &*self.0.borrow() {
            Selection::Range(r) => Some(r.clone()),
            Selection::Node(_) => None,
        }
    }

    pub(crate) fn with_range<R>(&self, f: impl FnOnce(&mut RangeSelection) -> R) -> Option<R> {
        match &mut *self.0.borrow_mut() {
            Selection::Range(r) => Some(f(r)),
            Selection::Node(_) => None,
        }
    }

    pub(crate) fn with_node<R>(&self, f: impl FnOnce(&mut NodeSelection) -> R) -> Option<R> {
        match &mut *self.0.borrow_mut() {
            Selection::Node(n) => Some(f(n)),
            Selection::Range(_) => None,
        }
    }

    /// A node selection's keys, in the order added; empty for a range
    /// selection.
    pub fn node_keys(&self) -> Vec<String> {
        self.with_node(|n| n.keys().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    /// The point `which` of a range selection.
    pub fn point(&self, which: Which) -> Option<Point> {
        self.with_range(|r| r.point(which).clone())
    }

    pub fn is_dirty(&self) -> bool {
        *self.0.borrow_mut().dirty_mut()
    }

    pub(crate) fn set_dirty(&self, dirty: bool) {
        *self.0.borrow_mut().dirty_mut() = dirty;
    }

    /// `selection.clone()`: a separate selection with the same value, not
    /// dirty.
    pub fn clone_selection(&self) -> SelectionRef {
        let mut value = self.get();
        match &mut value {
            Selection::Range(r) => {
                r.dirty = false;
                r.cached_is_backward = None;
            }
            Selection::Node(n) => n.dirty = false,
        }
        SelectionRef::new(value)
    }

    pub fn ptr_eq(&self, other: &SelectionRef) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// `point.set(key, offset, type)` on the point `which` of a range
    /// selection; nothing on a node selection.
    pub fn point_set(
        &self,
        which: Which,
        key: &str,
        offset: usize,
        point_type: PointType,
    ) -> Result<(), LexicalError> {
        // JS checks this in DEV builds, which the vectors come from.
        let node = get_node_by_key(key);
        let matches = match (&node, point_type) {
            (Some(n), PointType::Text) => n.is_text(),
            (Some(n), PointType::Element) => n.is_element(),
            (None, _) => false,
        };
        invariant!(
            matches,
            "PointType.set: node with key {key} is {} and can not be used for a {} point",
            node.map_or("[not found]", |n| n.get_type()),
            point_type.as_str()
        );
        let read_only = is_currently_read_only_mode();
        self.with_range(|r| {
            *r.point_mut(which) = Point::new(key, offset, point_type);
            // The composition key follows the point once composition is
            // ported; a headless editor never composes.
            if !read_only {
                r.cached_is_backward = None;
                r.dirty = true;
            }
        });
        Ok(())
    }

    /// `setTextNodeRange(anchorNode, anchorOffset, focusNode, focusOffset)`.
    pub(crate) fn set_text_node_range(
        &self,
        anchor: &NodeRef,
        anchor_offset: usize,
        focus: &NodeRef,
        focus_offset: usize,
    ) -> Result<(), LexicalError> {
        self.point_set(
            Which::Anchor,
            anchor.get_key(),
            anchor_offset,
            PointType::Text,
        )?;
        self.point_set(Which::Focus, focus.get_key(), focus_offset, PointType::Text)
    }

    /// `isBackward()`: whether the focus comes before the anchor.
    pub fn is_backward(&self) -> Result<bool, LexicalError> {
        let range = self.range().expect("a range selection");
        if let Some(cached) = range.cached_is_backward {
            return Ok(cached);
        }
        let is_backward = is_before(&range.focus, &range.anchor)?;
        if !is_currently_read_only_mode() {
            self.with_range(|r| r.cached_is_backward = Some(is_backward));
        }
        Ok(is_backward)
    }
}

/// `a.isBefore(b)`. Points on different nodes are ordered by carets
/// (`$comparePointCaretNext`), which come with the caret port.
fn is_before(a: &Point, b: &Point) -> Result<bool, LexicalError> {
    if a.key == b.key {
        return Ok(a.offset < b.offset);
    }
    Err(LexicalError::new(
        "Point.isBefore: points on different nodes need the caret port (rust/DESIGN.md, M1)",
    ))
}

/// `$getSelection()`.
pub fn get_selection() -> Option<SelectionRef> {
    get_active_editor_state().0.selection.borrow().clone()
}

/// `$setSelection(selection)`.
pub fn set_selection(selection: Option<SelectionRef>) {
    crate::updates::error_on_read_only();
    if let Some(selection) = &selection {
        selection.set_dirty(true);
    }
    *get_active_editor_state().0.selection.borrow_mut() = selection;
}

/// `$createRangeSelection()`: a selection at the start of the root, not
/// set on the editor state.
pub fn create_range_selection() -> SelectionRef {
    SelectionRef::new(Selection::Range(RangeSelection::new(
        Point::new("root", 0, PointType::Element),
        Point::new("root", 0, PointType::Element),
        0.0,
        "",
    )))
}

/// `$createNodeSelection()`.
pub fn create_node_selection() -> SelectionRef {
    SelectionRef::new(Selection::Node(NodeSelection::default()))
}

/// `$internalMakeRangeSelection`: a new range selection, set on the active
/// editor state.
pub(crate) fn internal_make_range_selection(anchor: Point, focus: Point) -> SelectionRef {
    let mut range = RangeSelection::new(anchor, focus, 0.0, "");
    range.dirty = true;
    let selection = SelectionRef::new(Selection::Range(range));
    *get_active_editor_state().0.selection.borrow_mut() = Some(selection.clone());
    selection
}

/// `selectPointOnNode(point, node)`.
fn select_point_on_node(
    selection: &SelectionRef,
    which: Which,
    node: &NodeRef,
) -> Result<(), LexicalError> {
    let mut key = node.get_key().to_owned();
    let mut offset = selection.point(which).expect("a range selection").offset;
    let mut point_type = PointType::Element;
    if node.is_text() {
        point_type = PointType::Text;
        offset = offset.min(node.get_text_content_size());
    } else if !node.is_element() {
        match node.get_next_sibling().filter(NodeRef::is_text) {
            Some(next) => {
                key = next.get_key().to_owned();
                offset = 0;
                point_type = PointType::Text;
            }
            None => {
                if let Some(parent) = node.get_parent() {
                    key = parent.get_key().to_owned();
                    offset = node.get_index_within_parent().map_or(0, |i| i + 1);
                }
            }
        }
    }
    selection.point_set(which, &key, offset, point_type)
}

/// `$moveSelectionPointToEnd(point, node)`.
pub(crate) fn move_selection_point_to_end(
    selection: &SelectionRef,
    which: Which,
    node: &NodeRef,
) -> Result<(), LexicalError> {
    if node.is_element() {
        match node.get_last_descendant() {
            Some(last) if last.is_element() || last.is_text() => {
                select_point_on_node(selection, which, &last)
            }
            _ => select_point_on_node(selection, which, node),
        }
    } else {
        select_point_on_node(selection, which, node)
    }
}

/// `$selectionTouchesElement(selection, parentNode)`.
pub(crate) fn selection_touches_element(selection: &SelectionRef, parent: &NodeRef) -> bool {
    selection
        .with_range(|r| r.anchor.key == parent.get_key() || r.focus.key == parent.get_key())
        .unwrap_or(false)
}

/// `$updateElementSelectionOnCreateDeleteNode(selection, parentNode,
/// nodeOffset, times)`.
pub(crate) fn update_element_selection_on_create_delete_node(
    selection: &SelectionRef,
    parent: &NodeRef,
    node_offset: usize,
    times: isize,
) -> Result<(), LexicalError> {
    if !selection_touches_element(selection, parent) {
        return Ok(());
    }
    for which in [Which::Anchor, Which::Focus] {
        let point = selection.point(which).expect("a range selection");
        if point.key == parent.get_key()
            && ((node_offset <= point.offset && times > 0)
                || (node_offset < point.offset && times < 0))
        {
            let offset = (point.offset as isize + times).max(0) as usize;
            selection.point_set(which, parent.get_key(), offset, PointType::Element)?;
        }
        let point = selection.point(which).expect("a range selection");
        let node = point.get_node()?;
        if node.is_element() {
            let size = node.get_children_size();
            let at_end = point.offset >= size;
            let child = if at_end {
                size.checked_sub(1).and_then(|i| node.get_child_at_index(i))
            } else {
                node.get_child_at_index(point.offset)
            };
            if let Some(child) = child.filter(NodeRef::is_text) {
                let offset = if at_end {
                    child.get_text_content_size()
                } else {
                    0
                };
                selection.point_set(which, child.get_key(), offset, PointType::Text)?;
            }
        }
    }
    Ok(())
}

/// `moveSelectionPointToSibling(point, node, parent, prevSibling,
/// nextSibling)`.
pub(crate) fn move_selection_point_to_sibling(
    selection: &SelectionRef,
    which: Which,
    node: &NodeRef,
    parent: &NodeRef,
    prev_sibling: Option<&NodeRef>,
    next_sibling: Option<&NodeRef>,
) -> Result<(), LexicalError> {
    let sibling = prev_sibling.or(next_sibling);
    match sibling.filter(|s| s.is_text() || s.is_element()) {
        Some(sibling) => {
            let is_text = sibling.is_text();
            let offset = if prev_sibling.is_none() {
                0
            } else if is_text {
                sibling.get_text_content_size()
            } else {
                sibling.get_children_size()
            };
            let point_type = if is_text {
                PointType::Text
            } else {
                PointType::Element
            };
            selection.point_set(which, sibling.get_key(), offset, point_type)
        }
        None => {
            let offset = node
                .get_index_within_parent()
                .unwrap_or_else(|| parent.get_children_size());
            selection.point_set(which, parent.get_key(), offset, PointType::Element)
        }
    }
}

/// `adjustPointOffsetForMergedSibling(point, isBefore, key, target,
/// textLength)`.
pub(crate) fn adjust_point_offset_for_merged_sibling(
    selection: &SelectionRef,
    which: Which,
    is_before: bool,
    key: &str,
    target: &NodeRef,
    text_length: usize,
) -> Result<(), LexicalError> {
    let point = selection.point(which).expect("a range selection");
    if point.point_type == PointType::Text {
        let offset = point.offset + if is_before { 0 } else { text_length };
        selection.point_set(which, key, offset, PointType::Text)
    } else if target
        .get_index_within_parent()
        .is_some_and(|i| point.offset > i)
    {
        selection.point_set(which, &point.key, point.offset - 1, PointType::Element)
    } else {
        Ok(())
    }
}

/// `$hasAncestor(child, targetNode)`.
fn has_ancestor(child: &NodeRef, target: &NodeRef) -> bool {
    let mut parent = child.get_parent();
    while let Some(p) = parent {
        if &p == target {
            return true;
        }
        parent = p.get_parent();
    }
    false
}

/// `$maybeMoveChildrenSelectionToParent(parentNode)`.
pub(crate) fn maybe_move_children_selection_to_parent(
    parent: &NodeRef,
) -> Result<Option<SelectionRef>, LexicalError> {
    let selection = get_selection();
    let Some(range) = selection.as_ref().filter(|s| s.is_range()) else {
        return Ok(selection);
    };
    if !parent.is_element() {
        return Ok(selection);
    }
    for which in [Which::Anchor, Which::Focus] {
        let node = range.point(which).expect("a range selection").get_node()?;
        if has_ancestor(&node, parent) {
            range.point_set(which, parent.get_key(), 0, PointType::Element)?;
        }
    }
    Ok(selection)
}

/// `editorStateHasDirtySelection(editorState, editor)`.
pub(crate) fn has_dirty_selection(
    pending: Option<&SelectionRef>,
    current: Option<&SelectionRef>,
) -> bool {
    match (pending, current) {
        (Some(pending), current) => {
            pending.is_dirty() || current.is_none_or(|c| !pending.get().is(&c.get()))
        }
        (None, current) => current.is_some(),
    }
}
