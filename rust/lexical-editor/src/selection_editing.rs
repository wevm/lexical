//! `RangeSelection`'s editing methods from `LexicalSelection.ts`: the
//! pending format and style, and `removeText`, which the other editing
//! methods build on.

use crate::LexicalError;
use crate::caret_utils::{
    SliceMode, caret_range_from_selection, remove_text_from_caret_range,
    update_range_selection_from_caret_range,
};
use crate::lexical_selection::{SelectionRef, get_selection, set_selection};
use crate::nodes::text::{to_int32, toggle_text_format_type};

impl SelectionRef {
    /// `toggleFormat(format)`: toggles a format on the selection's pending
    /// format, the one text typed at a collapsed selection gets.
    pub fn toggle_format(&self, format_type: &str) {
        self.with_range(|r| {
            r.format = f64::from(toggle_text_format_type(
                to_int32(r.format),
                format_type,
                None,
            ));
            r.dirty = true;
        });
    }

    /// `setFormat(format)`: sets the selection's pending format.
    pub fn set_format(&self, format: f64) {
        self.with_range(|r| {
            r.format = format;
            r.dirty = true;
        });
    }

    /// `setStyle(style)`: sets the selection's pending style.
    pub fn set_style(&self, style: &str) {
        self.with_range(|r| {
            r.style = style.to_owned();
            r.dirty = true;
        });
    }

    /// `hasFormat(type)`: whether the pending format has `format_type`.
    pub fn has_format(&self, format_type: &str) -> bool {
        let flag = toggle_text_format_type(0, format_type, None);
        self.with_range(|r| to_int32(r.format) & flag != 0)
            .unwrap_or(false)
    }

    /// `removeText()`: removes the selected content and collapses the
    /// selection where it was. When the caret lands in another node, the
    /// pending format and style come from that node.
    pub fn remove_text(&self) -> Result<(), LexicalError> {
        let Some(range) = self.range() else {
            return Ok(());
        };
        let is_current_selection = get_selection().is_some_and(|s| s.ptr_eq(self));
        let previous_anchor_key = range.anchor.key.clone();
        let new_range = remove_text_from_caret_range(
            &caret_range_from_selection(&range)?,
            SliceMode::RemoveEmptySlices,
        )?;
        update_range_selection_from_caret_range(self, &new_range)?;
        if self.range().is_some_and(|r| r.is_collapsed()) {
            internal_refresh_selection_format_and_style(self, &previous_anchor_key)?;
        }
        if is_current_selection && !get_selection().is_some_and(|s| s.ptr_eq(self)) {
            set_selection(Some(self.clone()));
        }
        Ok(())
    }
}

/// `$internalRefreshSelectionFormatAndStyle(selection, previousAnchorKey)`:
/// when the anchor moved to another node, takes the pending format and
/// style from the node it is in now.
pub(crate) fn internal_refresh_selection_format_and_style(
    selection: &SelectionRef,
    previous_anchor_key: &str,
) -> Result<(), LexicalError> {
    let Some(range) = selection.range() else {
        return Ok(());
    };
    if range.anchor.key == previous_anchor_key {
        return Ok(());
    }
    let anchor_node = range.anchor.get_node()?;
    let (format, style) = if anchor_node.is_text() {
        let data = anchor_node.text_data();
        (data.get_format(), data.get_style().to_owned())
    } else if anchor_node.is_element() {
        let data = anchor_node.element_data();
        (data.get_text_format(), data.get_text_style().to_owned())
    } else {
        (0.0, String::new())
    };
    selection.with_range(|r| {
        if r.format != format || r.style != style {
            r.format = format;
            r.style = style;
            r.dirty = true;
        }
    });
    Ok(())
}
