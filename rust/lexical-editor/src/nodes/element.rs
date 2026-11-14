//! `nodes/LexicalElementNode.ts`: the fields and base behavior of nodes that
//! have children.

use crate::caret_tree::{detach_node, detach_sibling_range, insert_node_between};
use crate::constants::{DOUBLE_LINE_BREAK, ELEMENT_TYPE_TO_FORMAT, lookup, name_of};
use crate::error::invariant;
use crate::lexical_selection::{
    Point, PointType, SelectionRef, Which, get_selection, internal_make_range_selection,
    move_selection_point_to_sibling,
};
use crate::lexical_utils::{get_writable, mutate, with_writable};
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::nodes::root::RootNode;
use crate::updates::error_on_read_only;
use crate::{LexicalError, utf16};
use std::collections::HashSet;

/// `ElementNode`'s text direction (`'ltr' | 'rtl'`; `null` is `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Ltr,
    Rtl,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Ltr => "ltr",
            Direction::Rtl => "rtl",
        }
    }
}

/// The fields `ElementNode` adds. Numbers a document can set
/// (`indent`, `textFormat`) are kept as the JS number they parsed to, so a
/// document writes back exactly what JS writes.
#[derive(Clone, Debug, Default)]
pub struct ElementData {
    pub(crate) first: Option<NodeKey>,
    pub(crate) last: Option<NodeKey>,
    pub(crate) size: usize,
    pub(crate) format: u8,
    pub(crate) style: String,
    pub(crate) indent: f64,
    pub(crate) dir: Option<Direction>,
    pub(crate) text_format: f64,
    pub(crate) text_style: String,
}

impl ElementData {
    /// `getFormatType()`: `''`, `'left'`, `'start'`, `'center'`, `'right'`,
    /// `'end'` or `'justify'`.
    pub fn get_format_type(&self) -> &'static str {
        name_of(ELEMENT_TYPE_TO_FORMAT, self.format).unwrap_or("")
    }

    /// `getFormat()`: the alignment flag.
    pub fn get_format(&self) -> u8 {
        self.format
    }

    pub fn get_style(&self) -> &str {
        &self.style
    }

    pub fn get_indent(&self) -> f64 {
        self.indent
    }

    pub fn get_direction(&self) -> Option<Direction> {
        self.dir
    }

    pub fn get_text_format(&self) -> f64 {
        self.text_format
    }

    pub fn get_text_style(&self) -> &str {
        &self.text_style
    }

    pub fn get_children_size(&self) -> usize {
        self.size
    }
}

/// `$isRootOrShadowRoot(node)`.
pub(crate) fn is_root_or_shadow_root(node: &dyn LexicalNode) -> bool {
    node.is::<RootNode>() || (node.element().is_some() && node.is_shadow_root())
}

/// `ElementNode.getTextContent()`. Slots, read first in JS, come with their
/// port.
pub fn get_text_content(node: &dyn LexicalNode) -> String {
    let children = NodeRef::new(node.get_key()).get_children();
    let length = children.len();
    let mut text_content = String::new();
    for (i, child) in children.iter().enumerate() {
        let child = child.get_latest();
        text_content.push_str(&child.get_text_content());
        if child.element().is_some() && i != length - 1 && !child.is_inline() {
            text_content.push_str(DOUBLE_LINE_BREAK);
        }
    }
    text_content
}

/// `ElementNode.shouldSerializeTextStyles()`: whether the element has no
/// text to derive its text format and style from, so they are written.
pub fn should_serialize_text_styles(node: &dyn LexicalNode) -> bool {
    if is_root_or_shadow_root(node) {
        return false;
    }
    !has_text_child(&NodeRef::new(node.get_key()), node.is_inline())
}

/// `$hasTextChild(element, deep)`. JS caches the deep walk during an export;
/// the result is the same without it.
fn has_text_child(element: &NodeRef, deep: bool) -> bool {
    if deep {
        return has_text_inside(element);
    }
    element.get_children().iter().any(NodeRef::is_text)
}

/// `$hasTextInside`.
fn has_text_inside(element: &NodeRef) -> bool {
    element
        .get_children()
        .iter()
        .any(|child| child.is_text() || (child.is_element() && has_text_inside(child)))
}

/// `isPointRemoved(point, nodesToRemoveKeySet, nodesToInsertKeySet)`.
fn is_point_removed(
    point: &Point,
    removed: &HashSet<&str>,
    inserted: &HashSet<&str>,
) -> Result<bool, LexicalError> {
    let mut node = Some(point.get_node()?);
    while let Some(n) = node {
        if removed.contains(n.get_key()) && !inserted.contains(n.get_key()) {
            return Ok(true);
        }
        node = n.get_parent();
    }
    Ok(false)
}

/// `ElementNode`'s methods, for a node that is an element. They panic on
/// any other node, as calling them on one is a bug.
impl NodeRef {
    fn with_element<R>(&self, f: impl FnOnce(&ElementData) -> R) -> R {
        let node = self.get_latest();
        f(node.element().expect("an element"))
    }

    fn set_element(&self, f: impl FnOnce(&mut ElementData)) {
        with_writable(self.get_key(), |n| f(n.element_mut().expect("an element")));
    }

    /// The element's fields (`getFormat()`, `getIndent()`, ...).
    pub fn element_data(&self) -> ElementData {
        self.with_element(Clone::clone)
    }

    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.get_children_size() == 0
    }

    /// `getChildAtIndex(index)`.
    pub fn get_child_at_index(&self, index: usize) -> Option<NodeRef> {
        let mut node = self.get_first_child();
        for _ in 0..index {
            node = node?.get_next_sibling();
        }
        node
    }

    /// `getFirstDescendant()`.
    pub fn get_first_descendant(&self) -> Option<NodeRef> {
        let mut node = self.get_first_child()?;
        while node.is_element() {
            match node.get_first_child() {
                Some(child) => node = child,
                None => break,
            }
        }
        Some(node)
    }

    /// `getLastDescendant()`.
    pub fn get_last_descendant(&self) -> Option<NodeRef> {
        let mut node = self.get_last_child()?;
        while node.is_element() {
            match node.get_last_child() {
                Some(child) => node = child,
                None => break,
            }
        }
        Some(node)
    }

    /// `ElementNode.getTextContentSize()`.
    pub(crate) fn get_element_text_content_size(&self) -> usize {
        let children = self.get_children();
        let length = children.len();
        let mut size = 0;
        for (i, child) in children.iter().enumerate() {
            size += child.get_text_content_size();
            if child.is_element() && i != length - 1 && !child.is_inline() {
                size += utf16::len(DOUBLE_LINE_BREAK);
            }
        }
        size
    }

    /// `ElementNode.select(anchorOffset, focusOffset)`: an element range at
    /// those child offsets, the end by default.
    pub fn select_element(
        &self,
        anchor_offset: Option<usize>,
        focus_offset: Option<usize>,
    ) -> Result<SelectionRef, LexicalError> {
        error_on_read_only();
        let selection = get_selection();
        let children_count = self.get_children_size();
        if !self.can_be_empty() {
            if anchor_offset == Some(0) && focus_offset == Some(0) {
                if let Some(first) = self
                    .get_first_child()
                    .filter(|c| c.is_text() || c.is_element())
                {
                    return first.select(Some(0), Some(0));
                }
            } else if anchor_offset.is_none_or(|o| o == children_count)
                && focus_offset.is_none_or(|o| o == children_count)
                && let Some(last) = self
                    .get_last_child()
                    .filter(|c| c.is_text() || c.is_element())
            {
                return last.select(None, None);
            }
        }
        let anchor_offset = anchor_offset.unwrap_or(children_count);
        let focus_offset = focus_offset.unwrap_or(children_count);
        let key = self.get_key();
        match selection.filter(SelectionRef::is_range) {
            None => Ok(internal_make_range_selection(
                Point::new(key, anchor_offset, PointType::Element),
                Point::new(key, focus_offset, PointType::Element),
            )),
            Some(selection) => {
                selection.point_set(Which::Anchor, key, anchor_offset, PointType::Element)?;
                selection.point_set(Which::Focus, key, focus_offset, PointType::Element)?;
                selection.set_dirty(true);
                Ok(selection)
            }
        }
    }

    /// `clear()`: removes every child.
    pub fn clear(&self) -> Result<(), LexicalError> {
        get_writable(self.get_key());
        for child in self.get_children() {
            child.remove(false)?;
        }
        Ok(())
    }

    /// `append(...nodesToAppend)`.
    pub fn append(&self, nodes: &[NodeRef]) -> Result<(), LexicalError> {
        self.get_latest().append(self, nodes)
    }

    /// `setDirection(direction)`.
    pub fn set_direction(&self, direction: Option<Direction>) {
        self.set_element(|e| e.dir = direction);
    }

    /// `setFormat(type)`: an alignment by name (`'center'`, ...); `''` and
    /// unknown names clear it.
    pub fn set_element_format(&self, format_type: &str) {
        let format = lookup(ELEMENT_TYPE_TO_FORMAT, format_type).unwrap_or(0);
        self.set_element(|e| e.format = format);
    }

    /// `setStyle(style)`.
    pub fn set_element_style(&self, style: &str) {
        self.set_element(|e| e.style = style.to_owned());
    }

    /// `setTextFormat(type)`.
    pub fn set_text_format(&self, text_format: f64) {
        self.set_element(|e| e.text_format = text_format);
    }

    /// `setTextStyle(style)`.
    pub fn set_text_style(&self, style: &str) {
        self.set_element(|e| e.text_style = style.to_owned());
    }

    /// `splice(start, deleteCount, nodesToInsert)`: removes `delete_count`
    /// children from `start` and inserts `nodes` there (moving any that are
    /// in the tree already), repairing the selection when children are
    /// removed.
    pub fn splice(
        &self,
        start: usize,
        delete_count: usize,
        nodes_to_insert: &[NodeRef],
    ) -> Result<(), LexicalError> {
        self.get_latest()
            .splice(self, start, delete_count, nodes_to_insert)
    }
}

/// `ElementNode.splice(start, deleteCount, nodesToInsert)`, which nodes that
/// override it call as `super.splice`. The root takes only elements
/// (`RootNode.splice`).
pub fn splice(
    this: &NodeRef,
    start: usize,
    delete_count: usize,
    nodes_to_insert: &[NodeRef],
) -> Result<(), LexicalError> {
    if this.is_root() {
        for node in nodes_to_insert {
            invariant!(
                node.is_element() || node.is_decorator(),
                "rootNode.splice: Only element or decorator nodes can be inserted to the root node"
            );
        }
    }
    let old_size = this.get_children_size();
    get_writable(this.get_key());
    invariant!(
        start + delete_count <= old_size,
        "ElementNode.splice: start + deleteCount > oldSize ({start} + {delete_count} > {old_size})"
    );
    let self_key = this.get_key();
    let mut nodes_to_insert_keys: Vec<String> = Vec::new();
    let mut nodes_to_remove_keys = Vec::new();
    let mut node_after_range = this.get_child_at_index(start + delete_count);
    let mut node_before_range = None;
    let mut new_size = (old_size - delete_count + nodes_to_insert.len()) as isize;

    if start != 0 {
        if start == old_size {
            node_before_range = this.get_last_child();
        } else if let Some(node) = this.get_child_at_index(start) {
            node_before_range = node.get_previous_sibling();
        }
    }

    if delete_count > 0 {
        let first = match &node_before_range {
            None => this.get_first_child(),
            Some(before) => before.get_next_sibling(),
        };
        nodes_to_remove_keys =
            detach_sibling_range(this, first, delete_count, node_before_range.as_ref())?;
    }

    // The writable insertion tail.
    let mut writable_prev_node = match &node_before_range {
        Some(before) if !nodes_to_insert.is_empty() => {
            get_writable(before.get_key());
            Some(before.clone())
        }
        _ => None,
    };
    for node_to_insert in nodes_to_insert {
        if writable_prev_node.as_ref() == Some(node_to_insert) {
            node_before_range = node_to_insert.get_previous_sibling();
            writable_prev_node = node_before_range.clone();
            if let Some(prev) = &writable_prev_node {
                get_writable(prev.get_key());
            }
        }
        if node_after_range.as_ref() == Some(node_to_insert) {
            node_after_range = node_to_insert.get_next_sibling();
        }
        get_writable(node_to_insert.get_key());
        if node_to_insert.get_parent().as_ref() == Some(this) {
            new_size -= 1;
        }
        detach_node(node_to_insert);
        if let Some(after) = &node_after_range {
            get_writable(after.get_key());
        }
        insert_node_between(
            self_key,
            node_to_insert.get_key(),
            writable_prev_node.as_ref().map(NodeRef::get_key),
            node_after_range.as_ref().map(NodeRef::get_key),
        );
        invariant!(
            node_to_insert.get_key() != self_key,
            "append: attempting to append this"
        );
        nodes_to_insert_keys.push(node_to_insert.get_key().to_owned());
        writable_prev_node = Some(node_to_insert.clone());
    }

    let new_size = usize::try_from(new_size).expect("size >= 0");
    mutate(self_key, |n| {
        n.element_mut().expect("an element").size = new_size
    });

    // Removing children moves the selection out of them, and removes an
    // element that can't be empty.
    if !nodes_to_remove_keys.is_empty()
        && let Some(selection) = get_selection().filter(SelectionRef::is_range)
    {
        let removed: HashSet<&str> = nodes_to_remove_keys.iter().map(String::as_str).collect();
        let inserted: HashSet<&str> = nodes_to_insert_keys.iter().map(String::as_str).collect();
        for which in [Which::Anchor, Which::Focus] {
            let point = selection.point(which).expect("a range selection");
            if is_point_removed(&point, &removed, &inserted)? {
                move_selection_point_to_sibling(
                    &selection,
                    which,
                    &point.get_node()?,
                    this,
                    node_before_range.as_ref(),
                    node_after_range.as_ref(),
                )?;
            }
        }
        if new_size == 0 && !this.can_be_empty() && !is_root_or_shadow_root(&*this.get_latest()) {
            this.remove(false)?;
        }
    }
    Ok(())
}

/// `ElementNode.exportDOM(editor)`: `LexicalNode`'s export, with the
/// indent (as `padding-inline-start` in steps of 40px and
/// `data-lexical-indent`) and the direction on an element.
#[cfg(feature = "html")]
pub fn export_dom(
    node: &dyn LexicalNode,
    this: &NodeRef,
    editor: &crate::Editor,
) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
    let output = crate::lexical_dom::export_dom_base(node, this, editor)?;
    if let Some(element) = output.element.as_ref().filter(|e| e.is_element()) {
        let data = node.element().expect("an element");
        let indent = data.get_indent();
        if indent > 0.0 {
            element.style().set_property(
                "padding-inline-start",
                &format!("{}px", crate::json::number_to_string(indent * 40.0)),
                "",
            );
            element.set_attribute(
                "data-lexical-indent",
                &crate::json::number_to_string(indent),
            );
        }
        if let Some(direction) = data.get_direction() {
            element.set_attribute("dir", direction.as_str());
        }
    }
    Ok(output)
}
