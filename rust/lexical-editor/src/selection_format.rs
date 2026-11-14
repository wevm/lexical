//! `formatText` from `LexicalSelection.ts` (`$formatText`,
//! `$updateTextFormat`, `$setTextFormat`, `$getCharacterOffsets`), with
//! `$splitTextPointCaretSlice` from `caret/LexicalCaretUtils.ts`, which
//! isolates the selected part of a text node.

use crate::LexicalError;
use crate::caret::TextPointCaretSlice;
use crate::caret_utils::{caret_range_from_selection, get_text_point_caret_slice_for_node};
use crate::constants::{TEXT_TYPE_TO_FORMAT, lookup};
use crate::lexical_selection::{
    PointType, SelectionRef, Which, get_selection, selection_touches_element,
    update_element_selection_on_create_delete_node,
};
use crate::node::NodeRef;
use crate::nodes::tab::TabNode;
use crate::nodes::text::{to_int32, toggle_text_format_type};
use crate::utf16;

impl SelectionRef {
    /// `selection.formatText(formatType, alignWithFormat)`
    /// (`$formatText(selection, formatType, alignWithFormat)`): toggles
    /// `format_type` on the selected text, splitting text nodes at the
    /// selection's edges, and aligns every node with the same target. On a
    /// collapsed selection (or one without text) it toggles the pending
    /// format instead.
    ///
    /// Without `align_with_format`, a range selection aligns with its
    /// pending format toggled, so the type is added unless the pending
    /// format has it; a node selection toggles each node on its own.
    pub fn format_text(
        &self,
        format_type: &str,
        align_with_format: Option<f64>,
    ) -> Result<(), LexicalError> {
        let effective_align = match (align_with_format, self.range()) {
            (None, Some(range)) => Some(f64::from(toggle_text_format_type(
                to_int32(range.format),
                format_type,
                None,
            ))),
            (align, _) => align,
        };
        update_text_format(self, |format| {
            toggle_format_number(format, format_type, effective_align)
        })
    }

    /// `$setTextFormat(selection, formats)`: sets (`true`) or clears
    /// (`false`) each format type on the selection, in order, so a later
    /// entry wins over an exclusive earlier one (subscript and superscript,
    /// the case transforms).
    pub fn set_text_format(&self, formats: &[(&str, bool)]) -> Result<(), LexicalError> {
        if formats.is_empty() {
            return Ok(());
        }
        update_text_format(self, |mut format| {
            for &(format_type, value) in formats {
                let align = if value {
                    lookup(TEXT_TYPE_TO_FORMAT, format_type).unwrap_or(0)
                } else {
                    0
                };
                format = toggle_format_number(format, format_type, Some(f64::from(align)));
            }
            format
        })
    }

    /// `$getCharacterOffsets(selection)`: the anchor's and focus's offsets
    /// in characters; an element point counts as the end of the element's
    /// text when it is after its last child, and 0 otherwise. A collapsed
    /// element selection and a node selection give `(0, 0)`.
    pub fn get_character_offsets(&self) -> Result<(usize, usize), LexicalError> {
        let Some(range) = self.range() else {
            return Ok((0, 0));
        };
        let (anchor, focus) = (&range.anchor, &range.focus);
        if anchor.point_type == PointType::Element
            && focus.point_type == PointType::Element
            && anchor.key == focus.key
            && anchor.offset == focus.offset
        {
            return Ok((0, 0));
        }
        // `getCharacterOffset(point)`
        let character_offset = |point: &crate::lexical_selection::Point| {
            if point.point_type == PointType::Text {
                return Ok(point.offset);
            }
            let parent = point.get_node()?;
            Ok(if point.offset == parent.get_children_size() {
                utf16::len(&parent.get_text_content())
            } else {
                0
            })
        };
        Ok((character_offset(anchor)?, character_offset(focus)?))
    }
}

/// `toggleTextFormatType(format, type, alignWithFormat)` on the JS number:
/// when `format` already agrees with the alignment it comes back unchanged
/// (not as `ToInt32`), as in JS.
fn toggle_format_number(format: f64, format_type: &str, align_with_format: Option<f64>) -> f64 {
    let align = align_with_format.map(to_int32);
    let active = lookup(TEXT_TYPE_TO_FORMAT, format_type).unwrap_or(0);
    if let Some(align) = align
        && to_int32(format) & active == align & active
    {
        return format;
    }
    f64::from(toggle_text_format_type(
        to_int32(format),
        format_type,
        align,
    ))
}

/// `$isTokenOrSegmented(node)`: a tab, or a text node in token or segmented
/// mode.
fn is_token_or_segmented(node: &NodeRef) -> bool {
    node.get_latest().is::<TabNode>() || node.is_token() || node.is_segmented()
}

/// `$updateTextFormat(selection, applyFormat)`: applies `apply_format` to
/// every formattable node, isolating partially selected text with caret
/// slices. Elements take it on their `textFormat`. When no text is
/// selected, it applies to the selection's pending format.
fn update_text_format(
    selection: &SelectionRef,
    apply_format: impl Fn(f64) -> f64,
) -> Result<(), LexicalError> {
    let Some(range) = selection.range() else {
        // A node selection. `$isInlineFormattable` holds for text nodes
        // (DecoratorTextNode is not ported).
        for node in selection.get_nodes()? {
            if node.is_text() {
                node.set_format(apply_format(node.text_data().get_format()));
            }
        }
        return Ok(());
    };

    let nodes = if range.is_collapsed() {
        Vec::new()
    } else {
        selection.get_nodes()?
    };
    let slices = if nodes.is_empty() {
        Vec::new()
    } else {
        caret_range_from_selection(&range)?
            .get_text_slices()
            .to_vec()
    };
    let mut has_text = false;
    let mut skipped_start: Option<Which> = None;
    let mut first_text: Option<NodeRef> = None;
    let mut first_format: Option<f64> = None;
    let mut last_format = 0.0;
    for node in &nodes {
        if node.is_text() {
            // Main's multi-text-node path moves a skipped text-edge start to
            // the first formatted node, including when the final slice is
            // empty.
            if let (Some(which), Some(first)) = (skipped_start, &first_text) {
                selection.point_set(which, first.get_key(), 0, PointType::Text)?;
                skipped_start = None;
            }
            let slice = get_text_point_caret_slice_for_node(&slices, node);
            let empty_slice = slice.is_some_and(|s| s.distance == 0);
            if !has_text && empty_slice {
                let start = if selection.is_backward()? {
                    Which::Focus
                } else {
                    Which::Anchor
                };
                let point = selection.point(start).expect("a range selection");
                if point.point_type == PointType::Text && point.key == node.get_key() {
                    skipped_start = Some(start);
                }
            }
            has_text = true;
            if empty_slice {
                continue;
            }
            let next_format = apply_format(node.text_data().get_format());
            let original_size = if skipped_start.is_some() {
                node.get_text_content_size()
            } else {
                0
            };
            let replacement = match slice {
                Some(slice) if !is_token_or_segmented(node) => {
                    split_text_point_caret_slice(slice, Some(selection))?
                }
                _ => Some(node.clone()),
            };
            if let Some(replacement) = replacement {
                replacement.set_format(next_format);
                if first_format.is_none() {
                    first_format = Some(next_format);
                    first_text = Some(replacement.clone());
                    // A single partial TextNode also repins the text
                    // endpoints after splitting. A whole-node or atomic-node
                    // selection keeps them.
                    if let Some(which) = skipped_start
                        && replacement.get_text_content_size() != original_size
                    {
                        selection.point_set(which, replacement.get_key(), 0, PointType::Text)?;
                        skipped_start = None;
                    }
                }
                last_format = next_format;
            }
        } else if node.is_element() {
            node.set_text_format(apply_format(node.element_data().get_text_format()));
        }
        // `$isInlineFormattable` holds for no other node here
        // (DecoratorTextNode is not ported).
    }
    if !has_text {
        let format = apply_format(range.format);
        selection.set_format(format);
        // `$setCompositionKey(null)`: composition is not ported.
    } else if let Some(first_format) = first_format {
        let format = f64::from(to_int32(first_format) | to_int32(last_format));
        selection.with_range(|r| r.format = format);
    }
    Ok(())
}

/// `$splitTextPointCaretSlice(slice, selection)`: splits the slice's text
/// node at both ends of the slice and returns the node holding exactly the
/// slice, or `None` for an empty slice (nothing changes). With
/// `selection`, its endpoints in the slice, including an element point just
/// before the node, stay on the returned node.
pub(crate) fn split_text_point_caret_slice(
    slice: &TextPointCaretSlice,
    selection: Option<&SelectionRef>,
) -> Result<Option<NodeRef>, LexicalError> {
    let origin = slice.caret.origin().clone();
    let (start, end) = slice.get_slice_indices();
    if start == end {
        return Ok(None);
    }
    if start == 0 && end == origin.get_text_content_size() {
        return Ok(Some(origin));
    }
    let parent = origin.get_parent();
    // Only element-point repair needs the sibling index. Text-only ranges
    // avoid an extra linear scan before splitText's own insertion lookup.
    let index = match (selection, &parent) {
        (Some(selection), Some(parent)) if selection_touches_element(selection, parent) => {
            origin.get_index_within_parent()
        }
        _ => None,
    };
    // Capture offsets before splitText repairs the active selection. In
    // particular, an element point before a prefix must stay on that prefix.
    let mut points = Vec::new();
    if let Some(selection) = selection {
        for which in [Which::Anchor, Which::Focus] {
            let point = selection.point(which).expect("a range selection");
            let offset = if point.key == origin.get_key() && point.point_type == PointType::Text {
                Some(point.offset.saturating_sub(start))
            } else if parent.as_ref().is_some_and(|p| point.key == p.get_key())
                && Some(point.offset) == index
            {
                Some(0)
            } else {
                None
            };
            points.push((which, offset));
        }
    }
    let split_nodes = origin.split_text(&[start as i64, end as i64])?;
    let node = split_nodes[if start == 0 { 0 } else { 1 }].clone();
    if let (Some(parent), Some(selection), Some(index)) = (&parent, selection, index)
        && !get_selection().is_some_and(|s| s.ptr_eq(selection))
    {
        update_element_selection_on_create_delete_node(
            selection,
            parent,
            index,
            split_nodes.len() as isize - 1,
        )?;
    }
    if let Some(selection) = selection {
        for (which, offset) in points {
            if let Some(offset) = offset {
                selection.point_set(which, node.get_key(), offset, PointType::Text)?;
            }
        }
    }
    Ok(Some(node))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexical_selection::{create_range_selection, set_selection};
    use crate::{
        CreateEditorArgs, Editor, UpdateOptions, create_editor, get_root, json,
        parse_serialized_node, reset_random_key,
    };

    fn discrete() -> UpdateOptions {
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        }
    }

    type Spec = (&'static str, usize, PointType);

    fn select(anchor: Spec, focus: Spec) -> Result<SelectionRef, LexicalError> {
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, anchor.0, anchor.1, anchor.2)?;
        selection.point_set(Which::Focus, focus.0, focus.1, focus.2)?;
        set_selection(Some(selection.clone()));
        Ok(selection)
    }

    /// The compact JSON, and the selection's anchor, focus and format.
    fn snapshot(editor: &Editor) -> (String, (Spec, Spec, f64)) {
        let state = editor.get_editor_state();
        let text = json::stringify(&state.to_json(true)).unwrap();
        let range = state
            .read(|| get_selection().and_then(|s| s.range()))
            .unwrap();
        let spec = |p: &crate::lexical_selection::Point| -> Spec {
            (p.key.clone().leak(), p.offset, p.point_type)
        };
        (
            text,
            (spec(&range.anchor), spec(&range.focus), range.format),
        )
    }

    /// `formatText` with and without `alignWithFormat` over a token node,
    /// a tab, element points and a skipped text-edge start,
    /// `$setTextFormat` with exclusive formats, and `$getCharacterOffsets`,
    /// as JS 0.53.0 gives them. (A collapsed toggle without alignment and a
    /// toggle over mixed formats are in `scripts/selection`.)
    #[test]
    fn format_text_matches_js() {
        use PointType::{Element as E, Text as T};
        reset_random_key();
        let editor = create_editor(CreateEditorArgs::default());
        editor
            .update(
                || {
                    let nodes = json::parse(
                        r#"[{"type":"paragraph","children":[{"type":"text","text":"abc","format":1},{"type":"text","text":"de","mode":"token"},{"type":"text","text":"fgh","format":2},{"type":"tab","text":"\t"}]},{"type":"paragraph","textFormat":4,"children":[{"type":"text","text":"ij"}]},{"type":"paragraph","children":[]}]"#,
                    )
                    .unwrap();
                    let nodes: Vec<NodeRef> = nodes
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
        let after_align = r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","format":1,"text":"abc"},{"type":"text","format":3,"mode":"token","text":"de"},{"type":"text","format":3,"text":"fgh"},{"type":"tab","format":1}],"textFormat":1},{"type":"paragraph","children":[{"type":"text","format":1,"text":"ij"}],"textFormat":1},{"type":"paragraph","children":[],"textFormat":1}]}}"#;
        let after_set = r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","format":1,"text":"a"},{"type":"text","format":64,"text":"bc"},{"type":"text","format":66,"mode":"token","text":"de"},{"type":"text","format":66,"text":"fgh"},{"type":"tab","format":64}],"textFormat":1},{"type":"paragraph","children":[{"type":"text","format":64,"text":"i"},{"type":"text","format":1,"text":"j"}],"textFormat":64},{"type":"paragraph","children":[],"textFormat":1}]}}"#;
        type Step = fn() -> Result<(), LexicalError>;
        let steps: [(Step, &str, (Spec, Spec, f64)); 4] = [
            (
                // Backward, from the end of "abc" (an empty slice) over a
                // token node to inside "fgh", which is already italic.
                || select(("4", 1, T), ("2", 3, T))?.format_text("italic", None),
                r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","format":1,"text":"abc"},{"type":"text","format":2,"mode":"token","text":"de"},{"type":"text","format":2,"text":"fgh"},{"type":"tab"}],"textFormat":1},{"type":"paragraph","children":[{"type":"text","text":"ij"}]},{"type":"paragraph","children":[]}]}}"#,
                (("4", 1, T), ("3", 0, T), 2.0),
            ),
            (
                || select(("1", 1, E), ("8", 0, E))?.format_text("bold", Some(1.0)),
                after_align,
                (("1", 1, E), ("8", 0, E), 3.0),
            ),
            (
                || select(("7", 1, T), ("7", 1, T))?.format_text("underline", Some(8.0)),
                after_align,
                (("7", 1, T), ("7", 1, T), 8.0),
            ),
            (
                || {
                    select(("2", 1, T), ("7", 1, T))?.set_text_format(&[
                        ("bold", false),
                        ("subscript", true),
                        ("superscript", true),
                    ])
                },
                after_set,
                (("10", 0, T), ("7", 1, T), 64.0),
            ),
        ];
        for (i, (step, state, selection)) in steps.into_iter().enumerate() {
            editor.update(step, discrete()).unwrap();
            assert_eq!(snapshot(&editor), (state.to_owned(), selection), "step {i}");
        }
        editor
            .update(
                || {
                    let offsets = select(("1", 5, E), ("7", 1, T))?.get_character_offsets()?;
                    assert_eq!(offsets, (9, 1));
                    let offsets = select(("1", 2, E), ("1", 2, E))?.get_character_offsets()?;
                    assert_eq!(offsets, (0, 0));
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
    }
}
