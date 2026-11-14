//! `insertText` from `LexicalSelection.ts` (`$insertText`,
//! `$insertTextAtPoint` and their helpers).
//!
//! A headless editor never composes, so `$getCompositionKey()` is always
//! `null` and `isComposing()` always false: the composition branches are
//! left out, each noted where JS has it. Without the text edit log or
//! NodeState, `$recordDeleted`, `$recordTextInserted` and `$carryNodeState`
//! do nothing.

use crate::LexicalError;
use crate::caret::{CaretDirection, get_sibling_caret};
use crate::error::invariant;
use crate::node::NodeRef;
use crate::nodes::element::is_root_or_shadow_root;
use crate::nodes::paragraph::{ParagraphNode, create_paragraph_node};
use crate::nodes::tab::TabNode;
use crate::nodes::text::create_text_node;
use crate::selection::{PointType, SelectionRef, Which, get_selection, set_selection};

impl SelectionRef {
    /// `selection.insertText(text)`: inserts `text` at a range selection,
    /// replacing what it covers; nothing for a node selection
    /// (`NodeSelection.insertText()`).
    pub fn insert_text(&self, text: &str) -> Result<(), LexicalError> {
        if !self.is_range() {
            return Ok(());
        }
        insert_text(self, text)
    }
}

impl NodeRef {
    /// `canInsertTextBefore()` of a text node or an element.
    pub fn can_insert_text_before(&self) -> bool {
        self.get_latest().can_insert_text_before()
    }

    /// `canInsertTextAfter()` of a text node or an element.
    pub fn can_insert_text_after(&self) -> bool {
        self.get_latest().can_insert_text_after()
    }
}

/// `$isTokenOrSegmented(node)`: a tab, or a text node in token or
/// segmented mode.
pub(crate) fn is_token_or_segmented(node: &NodeRef) -> bool {
    node.get_latest().is::<TabNode>() || node.is_token() || node.is_segmented()
}

/// `$isParagraphNode(node)`.
fn is_paragraph_node(node: &NodeRef) -> bool {
    node.get_latest().is::<ParagraphNode>()
}

/// `$insertText(selection, text, false)`.
fn insert_text(selection: &SelectionRef, text: &str) -> Result<(), LexicalError> {
    let range = selection.range().expect("a range selection");
    // For non-collapsed selections, inherit format/style from the first
    // selected text node so the replacement preserves the original styling.
    let mut format = range.format;
    let mut style = range.style.clone();
    if !range.is_collapsed() {
        let first_point = if range.focus.is_before(&range.anchor)? {
            &range.focus
        } else {
            &range.anchor
        };
        let first_node = first_point.get_node()?;
        if first_node.is_text() {
            let data = first_node.text_data();
            format = data.get_format();
            style = data.get_style().to_owned();
        }
        selection.remove_text()?;
        selection.with_range(|r| {
            r.format = format;
            r.style = style.clone();
        });
        if text.is_empty() {
            return Ok(());
        }
        // `$getCompositionKey()` is always null headless, so JS never falls
        // through to the collapsed code here.
        if selection.point(Which::Anchor).expect("a range").point_type == PointType::Element {
            transfer_starting_element_point_to_text_point(selection, format, &style)?;
        }
        return insert_text_at_point(selection, text, format, &style);
    }

    if range.anchor.point_type == PointType::Element {
        transfer_starting_element_point_to_text_point(selection, format, &style)?;
    }

    let anchor = selection.point(Which::Anchor).expect("a range selection");
    let anchor_node = anchor.get_node()?;
    invariant!(
        anchor_node.is_text(),
        "insertText: anchor is not a text node"
    );
    let offset = anchor.offset;

    let anchor_parent = anchor_node.get_parent_or_throw()?;
    let anchor_size = anchor_node.get_text_content_size();
    let needs_redirect = is_token_or_segmented(&anchor_node)
        || (offset == 0
            && (!anchor_node.can_insert_text_before()
                || (!anchor_parent.can_insert_text_before()
                    && anchor_node.get_previous_sibling().is_none())))
        || (offset == anchor_size
            && (!anchor_node.can_insert_text_after()
                || (!anchor_parent.can_insert_text_after()
                    && anchor_node.get_next_sibling().is_none())));

    if needs_redirect {
        // Token/segmented nodes and nodes whose parent forbids text insertion
        // at the boundary: reposition the cursor to an adjacent insertable
        // node, then recurse.

        // Segmented node with cursor in the middle: convert to normal
        // text node so that the recursive insertText can proceed.
        if anchor_node.is_segmented() && offset != 0 && offset != anchor_size {
            // JS sets the node's mode to normal instead while composing,
            // which a headless editor never does.
            let anchor_data = anchor_node.text_data();
            let replacement = create_text_node(anchor_data.get_text());
            replacement.set_format(format);
            replacement.set_style(&style);
            replacement.set_detail(anchor_data.get_detail())?;
            // `$carryNodeState`: NodeState is not ported.
            let is_current_selection = get_selection().is_some_and(|s| s.ptr_eq(selection));
            anchor_node.replace(&replacement, false)?;
            // replace() installs a clone of the active selection, so this
            // selection is still anchored to the segmented node that was just
            // detached. Move it onto the replacement and make it current
            // again, otherwise the recursive insertText below resolves an
            // anchor that is no longer in the tree.
            selection.set_text_node_range(&replacement, offset, &replacement, offset)?;
            if is_current_selection && !get_selection().is_some_and(|s| s.ptr_eq(selection)) {
                set_selection(Some(selection.clone()));
            }
            if !text.is_empty() {
                insert_text(selection, text)?;
            }
            return Ok(());
        }
        if text.is_empty() {
            return Ok(());
        }
        if offset == 0 || offset == anchor_size {
            let before = offset == 0;
            let direction = if before {
                CaretDirection::Previous
            } else {
                CaretDirection::Next
            };
            let sibling = get_sibling_caret(&anchor_node, direction).get_node_at_caret();
            let target = match sibling.filter(|sibling| {
                sibling.is_text()
                    && (if before {
                        sibling.can_insert_text_after()
                    } else {
                        sibling.can_insert_text_before()
                    })
                    && !is_token_or_segmented(sibling)
            }) {
                Some(sibling) => sibling,
                None => {
                    let target = create_text_node("");
                    target.set_format(format);
                    target.set_style(&style);
                    let can_insert = if before {
                        anchor_parent.can_insert_text_before()
                    } else {
                        anchor_parent.can_insert_text_after()
                    };
                    get_sibling_caret(
                        if can_insert {
                            &anchor_node
                        } else {
                            &anchor_parent
                        },
                        direction,
                    )
                    .insert(&target)?;
                    target
                }
            };
            let target_offset = if before { None } else { Some(0) };
            target.select(target_offset, target_offset)?;
            return insert_text(selection, text);
        }
        let new_node = create_text_node(text);
        new_node.set_format(format);
        new_node.set_style(&style);
        // `$recordDeleted` and `$recordTextInserted`: the text edit log is
        // not ported.
        anchor_node.replace(&new_node, false)?;
        new_node.select(None, None)?;
        return Ok(());
    }

    if text.is_empty() {
        return Ok(());
    }

    let at_start_of_inline =
        anchor_parent.is_inline() && offset == 0 && anchor_node.get_previous_sibling().is_none();
    let at_end_of_inline = anchor_parent.is_inline()
        && offset == anchor_size
        && anchor_node.get_next_sibling().is_none();
    let anchor_data = anchor_node.text_data();
    let format_differs = anchor_data.get_format() != format || anchor_data.get_style() != style;

    if at_start_of_inline || at_end_of_inline || format_differs {
        if anchor_data.get_text().is_empty() && !at_start_of_inline && !at_end_of_inline {
            anchor_node.set_format(format);
            anchor_node.set_style(&style);
        } else {
            return insert_text_at_point(selection, text, format, &style);
        }
    }

    anchor_node.splice_text(offset as i64, 0, text, true)?;
    // JS moves the anchor back over the text when the node is composing,
    // which it never is headless.
    Ok(())
}

/// `$insertTextAtPoint(selection, text, format, style)`: inserts `text` as
/// a new text node with `format` and `style` at the selection's anchor,
/// splitting the anchor's text node, and selects its end.
fn insert_text_at_point(
    selection: &SelectionRef,
    text: &str,
    format: f64,
    style: &str,
) -> Result<(), LexicalError> {
    let anchor = selection.point(Which::Anchor).expect("a range selection");
    let anchor_node = anchor.get_node()?;
    invariant!(
        anchor_node.is_text(),
        "insertText: anchor is not a text node"
    );
    let offset = anchor.offset;
    let text_node = create_text_node(text);
    text_node.set_format(format);
    text_node.set_style(style);
    // `$recordTextInserted`: the text edit log is not ported.
    let parent = anchor_node.get_parent_or_throw()?;
    let before = offset == 0;
    let at_boundary = before || offset == anchor_node.get_text_content_size();
    let adjacent = if before {
        anchor_node.get_previous_sibling()
    } else {
        anchor_node.get_next_sibling()
    };
    if at_boundary && parent.is_inline() && adjacent.is_none() {
        let direction = if before {
            CaretDirection::Previous
        } else {
            CaretDirection::Next
        };
        get_sibling_caret(&parent, direction).insert(&text_node)?;
    } else {
        let origin = if at_boundary {
            anchor_node.clone()
        } else {
            anchor_node.split_text(&[offset as i64])?.swap_remove(0)
        };
        if before {
            origin.insert_before(&text_node, false)?;
        } else {
            origin.insert_after(&text_node, false)?;
        }
    }
    if anchor_node.get_text_content().is_empty() && anchor_node.is_attached() {
        anchor_node.remove(false)?;
    }
    text_node.select_end()?;
    // JS moves the anchor back over the text when the node is composing,
    // which it never is headless.
    Ok(())
}

/// `$transferStartingElementPointToTextPoint(start, end, format, style)`
/// with the selection's anchor as `start` and its focus as `end`: puts an
/// empty text node with `format` and `style` where the element point is,
/// in a new paragraph where the element is a root, and moves the anchor
/// (and the focus, when it is the same point) into it.
fn transfer_starting_element_point_to_text_point(
    selection: &SelectionRef,
    format: f64,
    style: &str,
) -> Result<(), LexicalError> {
    let start = selection.point(Which::Anchor).expect("a range selection");
    let element = start.get_node()?;
    let placement_node = element.get_child_at_index(start.offset);
    let text_node = create_text_node("");
    text_node.set_format(format);
    text_node.set_style(style);
    if let Some(placement_node) = placement_node {
        if is_paragraph_node(&placement_node) {
            placement_node.splice(0, 0, std::slice::from_ref(&text_node))?;
        } else {
            // root or shadow-root + element-mode anchor before a
            // non-paragraph child (typically a sibling block decorator):
            // wrap the new text in a paragraph so it stays a valid
            // block-level child of the root or slot frame.
            let target = if is_root_or_shadow_root(&*element.get_latest()) {
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&text_node))?;
                paragraph
            } else {
                text_node.clone()
            };
            placement_node.insert_before(&target, true)?;
        }
    } else if is_root_or_shadow_root(&*element.get_latest()) {
        // root or shadow-root + last-offset typing: reuse the empty trailing
        // block when one exists instead of appending a fresh paragraph.
        match element.get_last_child() {
            Some(last_child)
                if last_child.is_element() && !last_child.is_inline() && last_child.is_empty() =>
            {
                last_child.append(std::slice::from_ref(&text_node))?;
            }
            _ => {
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&text_node))?;
                element.append(std::slice::from_ref(&paragraph))?;
            }
        }
    } else {
        element.append(std::slice::from_ref(&text_node))?;
    }
    // Transfer the element point to a text point.
    let range = selection.range().expect("a range selection");
    if range.anchor == range.focus {
        selection.point_set(Which::Focus, text_node.get_key(), 0, PointType::Text)?;
    }
    selection.point_set(Which::Anchor, text_node.get_key(), 0, PointType::Text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CreateEditorArgs, UpdateOptions, create_editor, get_root, json, parse_serialized_node,
        reset_random_key,
    };

    /// The redirects around token, segmented and tab nodes and the
    /// element-point transfer, which no vector reaches: the compact state
    /// and the selection JS 0.53.0 gives for the same document, point and
    /// text.
    #[test]
    fn insert_text_redirects_match_js() {
        // The paragraph's children, the collapsed point, the text, then the
        // state and the collapsed text point JS ends with.
        type Case = (
            &'static str,
            (&'static str, usize, PointType),
            &'static str,
            &'static str,
            (&'static str, usize),
        );
        let cases: &[Case] = &[
            (
                r#"{"type":"text","text":"a","mode":"token"},{"type":"text","text":"b","mode":"token"}"#,
                ("2", 1, PointType::Text),
                "X",
                r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","mode":"token","text":"a"},{"type":"text","text":"X"},{"type":"text","mode":"token","text":"b"}]}]}}"#,
                ("4", 1),
            ),
            (
                r#"{"type":"text","text":"ab cd","mode":"segmented","format":1,"detail":2}"#,
                ("2", 2, PointType::Text),
                "X",
                r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","detail":2,"text":"abX cd"}]}]}}"#,
                ("3", 3),
            ),
            (
                r#"{"type":"text","text":"a"},{"type":"tab"}"#,
                ("3", 1, PointType::Text),
                "X",
                r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","text":"a"},{"type":"tab"},{"type":"text","text":"X"}]}]}}"#,
                ("4", 1),
            ),
            (
                r#"{"type":"text","text":"a"}"#,
                ("root", 0, PointType::Element),
                "X",
                r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","text":"Xa"}]}]}}"#,
                ("3", 1),
            ),
            (
                r#"{"type":"text","text":"a"},{"type":"text","text":"b","format":1}"#,
                ("1", 1, PointType::Element),
                "X",
                r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","text":"aX"},{"type":"text","format":1,"text":"b"}]}]}}"#,
                ("2", 2),
            ),
        ];
        let discrete = || UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        };
        for &(children, (key, offset, point_type), text, state, (end_key, end_offset)) in cases {
            reset_random_key();
            let editor = create_editor(CreateEditorArgs::default());
            let doc = format!(r#"{{"type":"paragraph","children":[{children}]}}"#);
            editor
                .update(
                    move || {
                        let node = parse_serialized_node(&json::parse(&doc).unwrap())?;
                        get_root().append(&[node])
                    },
                    discrete(),
                )
                .unwrap();
            editor
                .update(
                    move || {
                        let selection = crate::selection::create_range_selection();
                        selection.point_set(Which::Anchor, key, offset, point_type)?;
                        selection.point_set(Which::Focus, key, offset, point_type)?;
                        set_selection(Some(selection.clone()));
                        selection.insert_text(text)
                    },
                    discrete(),
                )
                .unwrap();
            let editor_state = editor.get_editor_state();
            assert_eq!(
                json::stringify(&editor_state.to_json(true)).unwrap(),
                state,
                "{children}"
            );
            let range = editor_state.selection().unwrap().range().unwrap();
            let end = crate::selection::Point::new(end_key, end_offset, PointType::Text);
            assert_eq!(
                (range.anchor, range.focus),
                (end.clone(), end),
                "{children}"
            );
        }
    }
}
