//! `nodes/LexicalTextNode.ts`.

use crate::constants::{
    DETAIL_TYPE_TO_DETAIL, IS_DIRECTIONLESS, IS_NORMAL, IS_SEGMENTED, IS_TOKEN, IS_UNMERGEABLE,
    TEXT_MODE_TO_TYPE, TEXT_TYPE_TO_FORMAT, lookup, name_of,
};
use crate::error::invariant;
use crate::generated_json::{export_compact_text_node, export_text_node, update_text_node};
use crate::json::Object;
use crate::lexical_selection::{
    Point, PointType, SelectionRef, Which, adjust_point_offset_for_merged_sibling, get_selection,
    internal_make_range_selection, update_element_selection_on_create_delete_node,
};
use crate::lexical_utils::{
    get_writable, internal_mark_siblings_as_dirty, mutate, set_node_key, with_writable,
};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::tab::TabNode;
use crate::updates::error_on_read_only;
use crate::{LexicalError, utf16};

/// The fields `TextNode` adds. `format` and `detail` are kept as the JS
/// number they parsed to, so a document writes back exactly what JS writes;
/// JS's bitwise operators read them as `ToInt32`.
///
/// `text` is a Rust string: offsets into it are UTF-16 code units, as in JS,
/// converted where they are used (rust/DESIGN.md).
#[derive(Clone, Debug)]
pub struct TextData {
    pub(crate) text: String,
    pub(crate) format: f64,
    pub(crate) style: String,
    pub(crate) mode: u8,
    pub(crate) detail: f64,
}

impl Default for TextData {
    fn default() -> Self {
        Self {
            text: String::new(),
            format: 0.0,
            style: String::new(),
            mode: 0,
            detail: 0.0,
        }
    }
}

impl TextData {
    pub fn get_text(&self) -> &str {
        &self.text
    }

    pub fn get_format(&self) -> f64 {
        self.format
    }

    pub fn get_style(&self) -> &str {
        &self.style
    }

    pub fn get_detail(&self) -> f64 {
        self.detail
    }

    /// `getMode()`: `'normal'`, `'token'` or `'segmented'`.
    pub fn get_mode(&self) -> &'static str {
        name_of(TEXT_MODE_TO_TYPE, self.mode).unwrap_or("normal")
    }
}

#[derive(Clone, Debug, Default)]
pub struct TextNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for TextNode {
    fn get_type(&self) -> &'static str {
        "text"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_text_node(self.get_type(), &self.text)
        } else {
            export_text_node(self.get_type(), &self.text)
        }
    }
    fn update_from_json(&mut self, json: &Object) {
        update_text_node(&mut self.text, json);
    }
}

impl TextNode {
    /// A text node holding `text` (`new TextNode(text)`), not yet in an
    /// editor state.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            base: NodeBase::default(),
            text: TextData {
                text: text.into(),
                ..TextData::default()
            },
        }
    }
}

/// `$createTextNode(text)`.
pub fn create_text_node(text: &str) -> NodeRef {
    set_node_key(Box::new(TextNode::new(text)))
}

/// `ToInt32(x)`, how JS's bitwise operators read a number.
pub(crate) fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    x.trunc().rem_euclid(4_294_967_296.0) as u32 as i32
}

/// `toggleTextFormatType(format, type, alignWithFormat)`.
pub fn toggle_text_format_type(
    format: i32,
    format_type: &str,
    align_with_format: Option<i32>,
) -> i32 {
    let flag = |name| lookup(TEXT_TYPE_TO_FORMAT, name).unwrap_or(0);
    let active_format = flag(format_type);
    if let Some(align) = align_with_format
        && (format & active_format) == (align & active_format)
    {
        return format;
    }
    let mut new_format = format ^ active_format;
    match format_type {
        "subscript" => new_format &= !flag("superscript"),
        "superscript" => new_format &= !flag("subscript"),
        "lowercase" => new_format &= !(flag("uppercase") | flag("capitalize")),
        "uppercase" => new_format &= !(flag("lowercase") | flag("capitalize")),
        "capitalize" => new_format &= !(flag("lowercase") | flag("uppercase")),
        _ => {}
    }
    new_format
}

/// `text.slice(start)` with JS's negative `start` (from the end).
fn slice_from_signed(text: &str, start: i64) -> String {
    let start = if start < 0 {
        (utf16::len(text) as i64 + start).max(0)
    } else {
        start
    };
    utf16::slice_from(text, start as usize)
}

/// `TextNode`'s methods, for a node that is a text node (a `TextNode` or a
/// subclass such as `TabNode`). They panic on any other node, as calling
/// them on one is a bug.
impl NodeRef {
    fn with_text<R>(&self, f: impl FnOnce(&TextData) -> R) -> R {
        let node = self.get_latest();
        f(node.text().expect("a text node"))
    }

    fn set_text_data(&self, f: impl FnOnce(&mut TextData)) {
        with_writable(self.get_key(), |n| f(n.text_mut().expect("a text node")));
    }

    fn is_tab(&self) -> bool {
        self.get_latest().is::<TabNode>()
    }

    /// The text node's fields (`getFormat()`, `getStyle()`, ...).
    pub fn text_data(&self) -> TextData {
        self.with_text(Clone::clone)
    }

    /// `isSimpleText()`: a plain `text` node in normal mode.
    pub fn is_simple_text(&self) -> bool {
        self.get_type() == "text" && self.with_text(|t| t.mode == IS_NORMAL)
    }

    /// `isUnmergeable()`.
    pub fn is_unmergeable(&self) -> bool {
        self.with_text(|t| to_int32(t.detail) & IS_UNMERGEABLE != 0)
    }

    /// `isToken()`.
    pub fn is_token(&self) -> bool {
        self.with_text(|t| t.mode == IS_TOKEN)
    }

    /// `isSegmented()`.
    pub fn is_segmented(&self) -> bool {
        self.with_text(|t| t.mode == IS_SEGMENTED)
    }

    /// `hasFormat(type)`.
    pub fn has_format(&self, format_type: &str) -> bool {
        let flag = lookup(TEXT_TYPE_TO_FORMAT, format_type).unwrap_or(0);
        self.with_text(|t| to_int32(t.format) & flag != 0)
    }

    /// `setFormat(format)` with a number.
    pub fn set_format(&self, format: f64) {
        self.set_text_data(|t| t.format = format);
    }

    /// `setFormat(type)` with a name, such as `'bold'`.
    pub fn set_format_type(&self, format_type: &str) {
        let format = lookup(TEXT_TYPE_TO_FORMAT, format_type).unwrap_or(0);
        self.set_format(f64::from(format));
    }

    /// `setDetail(detail)` with a number. A tab only takes
    /// `IS_UNMERGEABLE`, which it has.
    pub fn set_detail(&self, detail: f64) -> Result<(), LexicalError> {
        if self.is_tab() {
            invariant!(
                detail == f64::from(IS_UNMERGEABLE),
                "TabNode does not support setDetail"
            );
            return Ok(());
        }
        self.set_text_data(|t| t.detail = detail);
        Ok(())
    }

    /// `setDetail(type)` with a name: `'directionless'` or `'unmergeable'`.
    pub fn set_detail_type(&self, detail_type: &str) -> Result<(), LexicalError> {
        invariant!(!self.is_tab(), "TabNode does not support setDetail");
        let detail = lookup(DETAIL_TYPE_TO_DETAIL, detail_type).unwrap_or(0);
        self.set_detail(f64::from(detail))
    }

    /// `setStyle(style)`.
    pub fn set_style(&self, style: &str) {
        self.set_text_data(|t| t.style = style.to_owned());
    }

    /// `toggleFormat(type)`.
    pub fn toggle_format(&self, format_type: &str) {
        let format = to_int32(self.with_text(|t| t.format));
        let new_format = toggle_text_format_type(format, format_type, None);
        self.set_format(f64::from(new_format));
    }

    /// `toggleDirectionless()`.
    pub fn toggle_directionless(&self) {
        self.set_text_data(|t| t.detail = f64::from(to_int32(t.detail) ^ IS_DIRECTIONLESS));
    }

    /// `toggleUnmergeable()`.
    pub fn toggle_unmergeable(&self) {
        self.set_text_data(|t| t.detail = f64::from(to_int32(t.detail) ^ IS_UNMERGEABLE));
    }

    /// `setMode(type)`: `'normal'`, `'token'` or `'segmented'`. A tab is
    /// always normal.
    pub fn set_mode(&self, mode_type: &str) -> Result<(), LexicalError> {
        if self.is_tab() {
            invariant!(mode_type == "normal", "TabNode does not support setMode");
            return Ok(());
        }
        let Some(mode) = lookup(TEXT_MODE_TO_TYPE, mode_type) else {
            return Err(LexicalError::new(format!(
                "setMode: unknown mode {mode_type}"
            )));
        };
        if self.with_text(|t| t.mode) != mode {
            self.set_text_data(|t| t.mode = mode);
        }
        Ok(())
    }

    /// `setTextContent(text)`. A tab keeps its `\t`.
    pub fn set_text_content(&self, text: &str) {
        let text = if self.is_tab() { "\t" } else { text };
        if self.with_text(|t| t.text != text) {
            self.set_text_data(|t| t.text = text.to_owned());
        }
    }

    /// `TextNode.select(anchorOffset, focusOffset)`: a text range at those
    /// UTF-16 offsets, the end by default.
    pub fn select_text(
        &self,
        anchor_offset: Option<usize>,
        focus_offset: Option<usize>,
    ) -> Result<SelectionRef, LexicalError> {
        error_on_read_only();
        let selection = get_selection();
        let last_offset = utf16::len(&self.get_text_content());
        let anchor_offset = anchor_offset.unwrap_or(last_offset);
        let focus_offset = focus_offset.unwrap_or(last_offset);
        let key = self.get_key();
        match selection.filter(SelectionRef::is_range) {
            None => Ok(internal_make_range_selection(
                Point::new(key, anchor_offset, PointType::Text),
                Point::new(key, focus_offset, PointType::Text),
            )),
            Some(selection) => {
                selection.set_text_node_range(self, anchor_offset, self, focus_offset)?;
                Ok(selection)
            }
        }
    }

    /// `spliceText(offset, delCount, newText, moveSelection)`: replaces
    /// `del_count` UTF-16 units at `offset` with `new_text`, and moves a
    /// range selection after it when `move_selection`.
    pub fn splice_text(
        &self,
        offset: i64,
        del_count: i64,
        new_text: &str,
        move_selection: bool,
    ) -> Result<NodeRef, LexicalError> {
        if self.is_tab() {
            invariant!(
                (new_text.is_empty() && del_count == 0) || (new_text == "\t" && del_count == 1),
                "TabNode does not support spliceText"
            );
            return Ok(self.clone());
        }
        get_writable(self.get_key());
        let text = self.get_text_content();
        let handled_text_length = utf16::len(new_text) as i64;
        let mut index = offset;
        if index < 0 {
            index = (handled_text_length + index).max(0);
        }
        if move_selection && let Some(selection) = get_selection().filter(SelectionRef::is_range) {
            let new_offset = (offset + handled_text_length).max(0) as usize;
            selection.set_text_node_range(self, new_offset, self, new_offset)?;
        }
        let updated_text = utf16::slice(&text, 0, index as usize)
            + new_text
            + &slice_from_signed(&text, index + del_count);
        mutate(self.get_key(), |n| {
            n.text_mut().expect("a text node").text = updated_text
        });
        Ok(self.clone())
    }

    /// `splitText(...splitOffsets)`: splits the text at those UTF-16
    /// offsets into this node and new siblings with the same format, style
    /// and detail, and returns the parts. A segmented node is replaced.
    pub fn split_text(&self, split_offsets: &[i64]) -> Result<Vec<NodeRef>, LexicalError> {
        error_on_read_only();
        let text_content = self.get_text_content();
        if text_content.is_empty() {
            return Ok(Vec::new());
        }
        let key = self.get_key().to_owned();
        let text_length = utf16::len(&text_content) as i64;
        let mut offsets = split_offsets.to_vec();
        offsets.sort_unstable();
        offsets.push(text_length);
        let mut parts = Vec::new();
        let mut start = 0;
        for &end in &offsets {
            if start >= text_length {
                break;
            }
            if end > start {
                parts.push(utf16::slice(&text_content, start as usize, end as usize));
                start = end;
            }
        }
        if parts.len() == 1 {
            return Ok(vec![self.clone()]);
        }
        let first_part = &parts[0];
        let parent = self.get_parent();
        let TextData {
            format,
            style,
            detail,
            ..
        } = self.text_data();

        // Prepare to handle selection
        let selection = get_selection().filter(SelectionRef::is_range);
        let mut start_text_point = None;
        let mut end_text_point = None;
        if let Some(selection) = &selection {
            let (start_point, end_point) = if selection.is_backward()? {
                (Which::Focus, Which::Anchor)
            } else {
                (Which::Anchor, Which::Focus)
            };
            let is_here = |which| {
                let point = selection.point(which).expect("a range selection");
                point.point_type == PointType::Text && point.key == key
            };
            if is_here(start_point) {
                start_text_point = Some(start_point);
            }
            if is_here(end_point) {
                end_text_point = Some(end_point);
            }
        }

        let copy_fields = |node: &NodeRef| {
            mutate(node.get_key(), |n| {
                let t = n.text_mut().expect("a text node");
                t.format = format;
                t.style = style.clone();
                t.detail = detail;
            });
        };
        let has_replaced_self = self.is_segmented();
        let writable_node = if has_replaced_self {
            let node = create_text_node(first_part);
            copy_fields(&node);
            node
        } else {
            self.set_text_content(first_part);
            self.clone()
        };

        let mut split_nodes = vec![writable_node];
        for part in &parts[1..] {
            let sibling = create_text_node(part);
            copy_fields(&sibling);
            split_nodes.push(sibling);
        }

        // Move the selection to the best location in the split string. The
        // end point is always left-biased, and the start point is generally
        // left-biased unless the end point would land on a later node.
        let offset_of = |which: Option<Which>| {
            which.map(|w| {
                selection
                    .as_ref()
                    .and_then(|s| s.point(w))
                    .expect("a range selection")
                    .offset
            })
        };
        let original_start_offset = offset_of(start_text_point);
        let original_end_offset = offset_of(end_text_point);
        let mut start_offset = 0;
        for node in &split_nodes {
            if start_text_point.is_none() && end_text_point.is_none() {
                break;
            }
            let selection = selection.as_ref().expect("a point is selected");
            let end_offset = start_offset + node.get_text_content_size();
            if let (Some(which), Some(original)) = (start_text_point, original_start_offset)
                && original <= end_offset
                && original >= start_offset
            {
                selection.point_set(
                    which,
                    node.get_key(),
                    original - start_offset,
                    PointType::Text,
                )?;
                if original < end_offset {
                    // The start isn't on a border, so we can stop checking.
                    start_text_point = None;
                }
            }
            if let (Some(which), Some(original)) = (end_text_point, original_end_offset)
                && original <= end_offset
                && original >= start_offset
            {
                selection.point_set(
                    which,
                    node.get_key(),
                    original - start_offset,
                    PointType::Text,
                )?;
                break;
            }
            start_offset = end_offset;
        }

        // Insert the nodes into the parent's children.
        if let Some(parent) = parent {
            internal_mark_siblings_as_dirty(self);
            get_writable(parent.get_key());
            let insertion_index = self.get_index_within_parent().expect("a child");
            if has_replaced_self {
                parent.splice(insertion_index, 0, &split_nodes)?;
                self.remove(false)?;
            } else {
                parent.splice(insertion_index, 1, &split_nodes)?;
            }
            if let Some(selection) = &selection {
                update_element_selection_on_create_delete_node(
                    selection,
                    &parent,
                    insertion_index,
                    parts.len() as isize - 1,
                )?;
            }
        }
        Ok(split_nodes)
    }

    /// `mergeWithSibling(target)`: appends (or prepends) the text of an
    /// adjacent text node and removes it, keeping the selection on the same
    /// text.
    pub fn merge_with_sibling(&self, target: &NodeRef) -> Result<NodeRef, LexicalError> {
        let is_before = self.get_previous_sibling().as_ref() == Some(target);
        invariant!(
            is_before || self.get_next_sibling().as_ref() == Some(target),
            "mergeWithSibling: sibling must be a previous or next sibling"
        );
        let key = self.get_key();
        let target_key = target.get_key();
        let text = self.get_text_content();
        let text_length = utf16::len(&text);
        if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
            for which in [Which::Anchor, Which::Focus] {
                if selection.point(which).expect("a range selection").key == target_key {
                    adjust_point_offset_for_merged_sibling(
                        &selection,
                        which,
                        is_before,
                        key,
                        target,
                        text_length,
                    )?;
                }
            }
        }
        let target_text = target.get_text_content();
        let new_text = if is_before {
            target_text + &text
        } else {
            text + &target_text
        };
        self.set_text_content(&new_text);
        get_writable(key);
        target.remove(false)?;
        Ok(self.clone())
    }
}
