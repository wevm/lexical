//! `nodes/LexicalElementNode.ts`: the fields and base behavior of nodes that
//! have children.

use crate::constants::{DOUBLE_LINE_BREAK, ELEMENT_TYPE_TO_FORMAT, name_of};
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::nodes::root::RootNode;

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
