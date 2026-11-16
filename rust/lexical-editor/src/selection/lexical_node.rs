//! `packages/lexical-selection/src/lexical-node.ts`: text slices, the
//! anchor helpers and the style patches.

use super::utils::get_css_from_style_object;
use crate::caret::TextPointCaretSlice;
use crate::caret_utils::{caret_range_from_selection, get_text_point_caret_slice_for_node};
use crate::error::invariant;
use crate::lexical_selection::get_selection;
use crate::lexical_utils::get_node_by_key;
use crate::node::LexicalNode;
use crate::nodes::text::create_text_node;
use crate::selection_format::split_text_point_caret_slice;
use crate::selection_insert_nodes::get_previous_selection;
use crate::selection_insert_text::is_token_or_segmented;
use crate::set_dom_style::get_style_object_from_css;
use crate::{Editor, LexicalError, NodeRef, Point, PointType, SelectionRef, Which, utf16};
use std::collections::HashSet;
use std::rc::Rc;

/// `$sliceSelectedTextNodeContent(selection, textNode, 'clone')`: the text
/// node, or, when the selection starts or ends inside it, a clone of it
/// holding only the selected text, which is in no editor state. What a copy
/// serializes for a text node at the edge of the selection.
///
/// JS's `'self'` mode, which writes the node's text without `getWritable`,
/// has no counterpart: a node's committed value can't change in place here,
/// and the ported callers all pass `'clone'`.
pub fn slice_selected_text_node_content(
    selection: &SelectionRef,
    text_node: &NodeRef,
) -> Result<Rc<dyn LexicalNode>, LexicalError> {
    let latest = text_node.get_latest();
    // `getStartEndPoints()`: `null` for a node selection.
    let Some(range) = selection.range() else {
        return Ok(latest);
    };
    let key = text_node.get_key();
    // Only the node of a text point can be cut short; checking that first
    // keeps a caller that slices every selected node from working out the
    // selection for each of them.
    if !((range.anchor.key == key || range.focus.key == key)
        && text_node.is_selected(Some(selection))?
        && !is_token_or_segmented(text_node))
    {
        return Ok(latest);
    }
    let (start, end) = if selection.is_backward()? {
        (&range.focus, &range.anchor)
    } else {
        (&range.anchor, &range.focus)
    };
    // Text endpoints can trim directly. Element endpoints need caret
    // normalization so equivalent mixed points produce an empty interval.
    let slices = if start.point_type == PointType::Element || end.point_type == PointType::Element {
        Some(caret_range_from_selection(&range)?.get_text_slices())
    } else {
        None
    };
    let slice = slices
        .as_ref()
        .and_then(|slices| get_text_point_caret_slice_for_node(slices, text_node));
    let (start_offset, end_offset) = match slice {
        Some(slice) => {
            let (start, end) = slice.get_slice_indices();
            (start, Some(end))
        }
        None => (
            if start.key == key { start.offset } else { 0 },
            (end.key == key).then_some(end.offset),
        ),
    };
    let current = &latest.text().expect("a text node").text;
    let text = utf16::slice(current, start_offset, end_offset.unwrap_or(usize::MAX));
    if text == *current {
        return Ok(latest);
    }
    // `$cloneWithPropertiesEphemeral`: a clone that is in no editor state.
    let mut clone = latest.clone_node();
    clone.text_mut().expect("a text node").text = text;
    Ok(Rc::from(clone))
}

/// `$isAtNodeEnd(point)`: whether the point is at the end of its text node,
/// or after the last child of its element.
pub fn is_at_node_end(point: &Point) -> Result<bool, LexicalError> {
    let node = point.get_node()?;
    if point.point_type == PointType::Text {
        return Ok(point.offset == node.get_text_content_size());
    }
    invariant!(
        node.is_element(),
        "isAtNodeEnd: node must be a TextNode or ElementNode"
    );
    Ok(point.offset == node.get_children_size())
}

/// `$trimTextContentFromAnchor(editor, anchor, delCount)`: removes
/// `del_count` UTF-16 code units of content before `anchor` (an empty block
/// counts as two), as a character limit does: whole nodes, then the end of
/// a text node, which goes back to its committed text instead where that
/// text is shorter. (JS also exports it as `trimTextContentFromAnchor`.)
pub fn trim_text_content_from_anchor(
    editor: &Editor,
    anchor: &Point,
    del_count: usize,
) -> Result<(), LexicalError> {
    // Work from the current selection anchor point
    let mut current_node = Some(anchor.get_node()?);
    let mut remaining = del_count as isize;
    if let Some(node) = current_node.clone().filter(NodeRef::is_element)
        && let Some(descendant) = node.get_descendant_by_index(anchor.offset)
    {
        current_node = Some(descendant);
    }
    while remaining > 0 {
        let Some(mut node) = current_node.clone() else {
            break;
        };
        if node.is_element()
            && let Some(last_descendant) = node.get_last_descendant()
        {
            node = last_descendant;
        }
        let mut next_node = node.get_previous_sibling();
        let mut additional_element_whitespace = 0;
        if next_node.is_none() {
            let mut parent = Some(node.get_parent_or_throw()?);
            let mut parent_sibling = parent.as_ref().and_then(NodeRef::get_previous_sibling);
            while parent_sibling.is_none() {
                parent = parent.and_then(|p| p.get_parent());
                let Some(p) = &parent else {
                    next_node = None;
                    break;
                };
                parent_sibling = p.get_previous_sibling();
            }
            if let Some(parent) = &parent {
                additional_element_whitespace = if parent.is_inline() { 0 } else { 2 };
                next_node = parent_sibling;
            }
        }
        let mut text = node.get_text_content();
        // If the text is empty, we need to consider adding in two line breaks to match
        // the content if we were to get it from its parent.
        if text.is_empty() && node.is_element() && !node.is_inline() {
            text = "\n\n".to_owned();
        }
        let current_node_size = utf16::len(&text) as isize;
        if !node.is_text() || remaining >= current_node_size {
            let parent = node.get_parent();
            node.remove(false)?;
            if let Some(parent) = parent
                && parent.get_children_size() == 0
                && !parent.is_root()
            {
                parent.remove(false)?;
            }
            remaining -= current_node_size + additional_element_whitespace;
            current_node = next_node;
            continue;
        }
        let key = node.get_key().to_owned();
        // See if we can just revert it to what was in the last editor state
        let prev_text_content = editor.get_editor_state().read(|| {
            get_node_by_key(&key)
                .filter(|prev| prev.is_text() && prev.is_simple_text())
                .map(|prev| prev.get_text_content())
        });
        let offset = current_node_size - remaining;
        if let Some(prev_text_content) = prev_text_content.filter(|prev| *prev != text) {
            let prev_selection = get_previous_selection();
            let mut target = node.clone();
            if !node.is_simple_text() {
                let text_node = create_text_node(&prev_text_content);
                node.replace(&text_node, false)?;
                target = text_node;
            } else {
                node.set_text_content(&prev_text_content);
            }
            if let Some(prev) = prev_selection.and_then(|s| s.range())
                && prev.is_collapsed()
            {
                let prev_offset = prev.anchor.offset;
                target.select(Some(prev_offset), Some(prev_offset))?;
            }
        } else if node.is_simple_text() {
            // Split text
            let is_selected = anchor.key == key;
            let mut anchor_offset = anchor.offset as isize;
            // Move offset to end if it's less than the remaining number, otherwise
            // we'll have a negative splitStart.
            if anchor_offset < remaining {
                anchor_offset = current_node_size;
            }
            let split_start = if is_selected {
                anchor_offset - remaining
            } else {
                0
            };
            let split_end = if is_selected { anchor_offset } else { offset };
            let parts = node.split_text(&[split_start as i64, split_end as i64])?;
            let excess_node = if is_selected && split_start == 0 {
                parts.first()
            } else {
                parts.get(1)
            };
            excess_node
                .ok_or_else(|| LexicalError::new("$trimTextContentFromAnchor: no text to remove"))?
                .remove(false)?;
        } else {
            let text_node = create_text_node(&utf16::slice(&text, 0, offset as usize));
            node.replace(&text_node, false)?;
        }
        remaining = 0;
    }
    Ok(())
}

/// `$addNodeStyle(node)`: does nothing but warn, once, that the calls
/// should be removed, as node styles are parsed on demand.
#[deprecated(note = "a no-op: node styles are parsed on demand")]
pub fn add_node_style(_node: &NodeRef) {
    crate::warnings::warn_only_once(
        "$addNodeStyle is a deprecated no-op and calls should be removed",
    );
}

/// What `$patchStyle` applies a patch to, as a patch function receives it:
/// a text node, a collapsed range selection or an element (its text style).
pub enum StylePatchTarget<'a> {
    Text(&'a NodeRef),
    Selection(&'a SelectionRef),
    Element(&'a NodeRef),
}

/// A function of a style patch: the property's current value (`None` where
/// JS passes `undefined`) and the target to the new value.
pub type StylePatchFn = dyn Fn(Option<&str>, &StylePatchTarget<'_>) -> String;

/// A value of a style patch: a value, `null` (removing the property) or a
/// function of the current value.
#[derive(Clone)]
pub enum StylePatchValue {
    Value(String),
    Remove,
    Compute(Rc<StylePatchFn>),
}

impl From<&str> for StylePatchValue {
    fn from(value: &str) -> Self {
        StylePatchValue::Value(value.to_owned())
    }
}

/// `$patchStyle(target, patch)`: applies `patch`, in order (JS's is
/// `Object.entries(patch)`), to the target's style.
fn patch_style<K: AsRef<str>>(
    target: StylePatchTarget<'_>,
    patch: &[(K, StylePatchValue)],
) -> Result<(), LexicalError> {
    let prev_css = match &target {
        StylePatchTarget::Selection(selection) => {
            let range = selection.range().filter(|r| r.is_collapsed());
            invariant!(
                range.is_some(),
                "$patchStyle must only be called with a TextNode, ElementNode, or collapsed RangeSelection"
            );
            range.expect("checked above").style
        }
        StylePatchTarget::Text(node) => node.text_data().style,
        StylePatchTarget::Element(node) => node.element_data().get_text_style().to_owned(),
    };
    let prev_styles = get_style_object_from_css(&prev_css);
    let mut new_styles = prev_styles.clone();
    for (key, value) in patch {
        let key = key.as_ref();
        match value {
            StylePatchValue::Compute(f) => new_styles.insert(key, f(prev_styles.get(key), &target)),
            StylePatchValue::Remove => {
                new_styles.remove(key);
            }
            StylePatchValue::Value(value) => new_styles.insert(key, value.as_str()),
        }
    }
    let new_css_text = get_css_from_style_object(&new_styles);
    match target {
        StylePatchTarget::Selection(selection) => selection.set_style(&new_css_text),
        StylePatchTarget::Text(node) => node.set_style(&new_css_text),
        StylePatchTarget::Element(node) => node.set_text_style(&new_css_text),
    }
    Ok(())
}

/// `$patchStyleText(selection, patch)`: applies `patch` (in order) to the
/// style of the selected text, splitting a text node the selection covers
/// in part; to a collapsed range selection's style and the empty element it
/// is in; and to the text style of the empty elements the selection holds
/// that can be empty. No node is patched twice.
pub fn patch_style_text<K: AsRef<str>>(
    selection: &SelectionRef,
    patch: &[(K, StylePatchValue)],
) -> Result<(), LexicalError> {
    // Shared with the empty-element loop below so that a node patched here is
    // not patched a second time. A function-valued patch is not idempotent —
    // it is evaluated against the value the previous application wrote.
    let mut patched_element_keys: HashSet<String> = HashSet::new();
    if let Some(range) = selection.range()
        && range.is_collapsed()
    {
        patch_style(StylePatchTarget::Selection(selection), patch)?;
        let empty_node = range.anchor.get_node()?;
        if empty_node.is_element() && empty_node.is_empty() {
            patched_element_keys.insert(empty_node.get_key().to_owned());
            patch_style(StylePatchTarget::Element(&empty_node), patch)?;
        }
    }
    for_each_selected_text_node_in_selection(Some(selection), |text_node| {
        patch_style(StylePatchTarget::Text(text_node), patch)
    })?;
    for node in selection.get_nodes()? {
        if !node.is_element() || !node.can_be_empty() || node.get_children_size() != 0 {
            continue;
        }
        if !patched_element_keys.insert(node.get_key().to_owned()) {
            continue;
        }
        patch_style(StylePatchTarget::Element(&node), patch)?;
    }
    Ok(())
}

/// `$forEachSelectedTextNode(fn)`: calls `f` with each text node of the
/// selection that can have a format and has selected text, split to the
/// selected part (unless it is a token or segmented).
pub fn for_each_selected_text_node(
    f: impl FnMut(&NodeRef) -> Result<(), LexicalError>,
) -> Result<(), LexicalError> {
    for_each_selected_text_node_in_selection(get_selection().as_ref(), f)
}

/// `$forEachSelectedTextNodeInSelection(selection, fn)`.
fn for_each_selected_text_node_in_selection(
    selection: Option<&SelectionRef>,
    mut f: impl FnMut(&NodeRef) -> Result<(), LexicalError>,
) -> Result<(), LexicalError> {
    let Some(selection) = selection else {
        return Ok(());
    };
    let slices: Vec<Option<TextPointCaretSlice>> = match selection.range() {
        Some(range) => caret_range_from_selection(&range)?
            .get_text_slices()
            .to_vec(),
        None => Vec::new(),
    };
    let range_selection = selection.is_range().then_some(selection);
    for node in selection.get_nodes()? {
        if !node.is_text() || !node.can_have_format() {
            continue;
        }
        let slice = get_text_point_caret_slice_for_node(&slices, &node);
        let empty = match slice {
            Some(slice) => slice.distance == 0,
            None => node.get_text_content_size() == 0,
        };
        if empty {
            continue;
        }
        match slice {
            Some(slice) if !is_token_or_segmented(&node) => {
                let split = split_text_point_caret_slice(slice, range_selection)?
                    .ok_or_else(|| LexicalError::new("an empty text slice"))?;
                f(&split)?;
            }
            _ => f(&node)?,
        }
    }
    // Preserve the existing single-text-node forward selection convention.
    if let Some(range) = selection.range()
        && range.anchor.point_type == PointType::Text
        && range.focus.point_type == PointType::Text
        && range.anchor.key == range.focus.key
    {
        ensure_forward_range_selection(selection)?;
    }
    Ok(())
}

/// `$ensureForwardRangeSelection(selection)`: swaps the points of a
/// backward range selection in place. Nothing for a node selection.
pub fn ensure_forward_range_selection(selection: &SelectionRef) -> Result<(), LexicalError> {
    let Some(range) = selection.range() else {
        return Ok(());
    };
    if selection.is_backward()? {
        let (anchor, focus) = (range.anchor, range.focus);
        selection.point_set(Which::Anchor, &focus.key, focus.offset, focus.point_type)?;
        selection.point_set(Which::Focus, &anchor.key, anchor.offset, anchor.point_type)?;
    }
    Ok(())
}
