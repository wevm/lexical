//! `packages/lexical-react/src/shared/useCharacterLimit.ts`: the logic of
//! `@lexical/react`'s `CharacterLimitPlugin`, without React's hook plumbing.
//! `useCharacterLimit`'s effects are [`register_character_limit`], which
//! returns their clean-up, as Swift's `registerCharacterLimit` does.

use super::grapheme::graphemes;
use super::{OverflowNode, create_overflow_node, is_overflow_node};
use crate::editor::Editor;
use crate::lexical_selection::{PointType, Which};
use crate::nodes::line_break::LineBreakNode;
use crate::text::root_text_content;
use crate::utils::{dfs_with_slots, find_matching_parent, unwrap_node};
use crate::{
    COMMAND_PRIORITY_LOW, DELETE_CHARACTER_COMMAND, HISTORY_MERGE_TAG, LexicalError, NodeRef,
    Unregister, UpdateOptions, get_selection, get_slot_host, merge_register, set_selection, utf16,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// How the character limit measures text (`strlen`): a length for a string.
pub type Strlen = Rc<dyn Fn(&str) -> i64>;

/// `updateCharacterLimit()`, given the editor.
type UpdateCharacterLimit = dyn Fn(&Editor) -> Result<(), LexicalError>;

/// `useCharacterLimit(editor, maxCharacters, {strlen, remainingCharacters})`:
/// wraps the text past `max_characters` in `OverflowNode`s and reports the
/// characters left (negative past the limit) to `remaining_characters`, at
/// once and after each change of content, and handles
/// `DELETE_CHARACTER_COMMAND` at low priority. `strlen` measures text, in
/// UTF-16 code units by default; the limit falls between grapheme clusters.
/// Fails, as JS's invariant does, when the editor doesn't register
/// `OverflowNode`. Returns the clean-up.
pub fn register_character_limit(
    editor: &Editor,
    max_characters: i64,
    strlen: Option<Strlen>,
    remaining_characters: Option<Rc<dyn Fn(i64)>>,
) -> Result<Unregister, LexicalError> {
    if !editor.registered_nodes().contains_key("overflow") {
        return Err(LexicalError::new(
            "useCharacterLimit: OverflowNode not registered on editor",
        ));
    }
    // UTF-16.
    let strlen: Strlen = strlen.unwrap_or_else(|| Rc::new(|input| utf16::len(input) as i64));
    let remaining_characters = remaining_characters.unwrap_or_else(|| Rc::new(|_| {}));

    let text = Rc::new(RefCell::new(
        editor.get_editor_state().read(root_text_content),
    ));
    let last_computed_text_length: Rc<Cell<Option<i64>>> = Rc::new(Cell::new(None));

    // Not `$`-prefixed in JS: it runs outside an update and opens its own
    // for the OverflowNode wrapping.
    let update_character_limit: Rc<UpdateCharacterLimit> = {
        let text = text.clone();
        Rc::new(move |editor: &Editor| {
            let text = text.borrow().clone();
            let text_length = strlen(&text);
            let text_length_above_threshold = text_length > max_characters
                || last_computed_text_length
                    .get()
                    .is_some_and(|last| last > max_characters);
            let diff = max_characters - text_length;
            remaining_characters(diff);
            let mut result = Ok(());
            if last_computed_text_length.get().is_none() || text_length_above_threshold {
                let offset = find_offset(&text, max_characters, &*strlen);
                result = editor.update(
                    move || wrap_overflowed_nodes(offset),
                    UpdateOptions {
                        tags: vec![HISTORY_MERGE_TAG.to_owned()],
                        ..UpdateOptions::default()
                    },
                );
            }
            last_computed_text_length.set(Some(text_length));
            result
        })
    };

    // Count the content already there before subscribing: an update listener
    // does not fire on registration.
    update_character_limit(editor)?;

    let weak = editor.weak();
    Ok(merge_register(vec![
        {
            let text = text.clone();
            editor.register_text_content_listener(move |current_text| {
                *text.borrow_mut() = current_text.to_owned();
            })
        },
        editor.register_update_listener(move |payload| {
            // `editor.isComposing()`: a headless editor never composes.
            let has_content_changes =
                !payload.dirty_leaves.is_empty() || !payload.dirty_elements.is_empty();
            if !has_content_changes {
                return;
            }
            if let Some(editor) = weak.upgrade().map(Editor) {
                // A listener has no error channel; the update it queues
                // reports its own failure.
                let _ = update_character_limit(&editor);
            }
        }),
        editor.register_command(
            &DELETE_CHARACTER_COMMAND,
            |is_backward, _| {
                let Some(selection) = get_selection().filter(|s| s.is_range()) else {
                    return Ok(false);
                };
                let anchor_node = selection
                    .point(Which::Anchor)
                    .expect("a range selection")
                    .get_node()?;
                let overflow = anchor_node.get_parent();
                let overflow_parent = overflow.and_then(|o| o.get_parent());
                let parent_next = overflow_parent.as_ref().and_then(NodeRef::get_next_sibling);
                selection.delete_character(*is_backward)?;
                match (&overflow_parent, &parent_next) {
                    (Some(overflow_parent), _) if overflow_parent.is_empty() => {
                        overflow_parent.remove(false)?;
                    }
                    (_, Some(parent_next))
                        if parent_next.is_element() && parent_next.is_empty() =>
                    {
                        parent_next.remove(false)?;
                    }
                    _ => {}
                }
                Ok(true)
            },
            COMMAND_PRIORITY_LOW,
        ),
    ]))
}

/// `findOffset(text, maxCharacters, strlen)` (module-private upstream): the
/// UTF-16 offset in `text` after the grapheme clusters that fit in
/// `max_characters` as `strlen` measures them. JS segments with
/// `Intl.Segmenter`, which Node always has; its fallback for an engine
/// without one (code points) has no counterpart.
pub fn find_offset(text: &str, max_characters: i64, strlen: &dyn Fn(&str) -> i64) -> usize {
    let mut offset_utf16 = 0;
    let mut offset = 0;
    for grapheme in graphemes(text) {
        let next_offset = offset + strlen(grapheme);
        if next_offset > max_characters {
            break;
        }
        offset = next_offset;
        offset_utf16 += utf16::len(grapheme);
    }
    offset_utf16
}

/// `$isLeafNode(node)`: a text node, a line break or a decorator.
fn is_leaf_node(node: &NodeRef) -> bool {
    node.is_text() || node.get_latest().is::<LineBreakNode>() || node.is_decorator()
}

/// `$wrapOverflowedNodes(offset)`: wraps the leaves past `offset` (a UTF-16
/// offset in the root's text content, slots included) in `OverflowNode`s,
/// and unwraps those that are within it again.
pub fn wrap_overflowed_nodes(offset: usize) -> Result<(), LexicalError> {
    // `$dfsWithSlots`, so slot hosts count their slots' text.
    let dfs_nodes = dfs_with_slots(None, None);
    let mut accumulated_length = 0;
    for dfs_node in dfs_nodes {
        let node = dfs_node.node;
        // `ElementNode.getTextContent()` puts "\n\n" after each block child
        // but the last, which the offset counts.
        if let Some(prev_sibling) = node.get_previous_sibling()
            && prev_sibling.is_element()
            && !prev_sibling.is_inline()
        {
            accumulated_length += 2;
        }
        // A slot value root has no parent to replace it in, so it is not
        // wrapped, but its text counts.
        let is_slot_value_leaf = is_leaf_node(&node) && get_slot_host(&node).is_some();
        let needs_overflow_parent = is_leaf_node(&node)
            && !is_slot_value_leaf
            && find_matching_parent(&node, is_overflow_node).is_none();

        if is_overflow_node(&node) {
            let previous_length = accumulated_length;
            let next_length = accumulated_length + node.get_text_content_size();
            if next_length <= offset {
                let parent = node.get_parent();
                let previous_sibling = node.get_previous_sibling();
                let next_sibling = node.get_next_sibling();
                unwrap_node(&node)?;
                // Restore the selection when the overflow's children are
                // removed.
                if let Some(selection) = get_selection().filter(|s| s.is_range()) {
                    let attached = |which| {
                        selection
                            .point(which)
                            .expect("a range selection")
                            .get_node()
                            .map(|n| n.is_attached())
                    };
                    if !attached(Which::Anchor)? || !attached(Which::Focus)? {
                        if let Some(previous_sibling) = previous_sibling.filter(NodeRef::is_text) {
                            previous_sibling.select(None, None)?;
                        } else if let Some(next_sibling) = next_sibling.filter(NodeRef::is_text) {
                            next_sibling.select(None, None)?;
                        } else if let Some(parent) = parent {
                            parent.select(None, None)?;
                        }
                    }
                }
            } else if previous_length < offset {
                let descendant = node.get_first_descendant();
                let descendant_length = descendant
                    .as_ref()
                    .map_or(0, NodeRef::get_text_content_size);
                let previous_plus_descendant_length = previous_length + descendant_length;
                // Simple text can be split to fit the overflow more closely.
                let first_descendant_is_simple_text = descendant
                    .as_ref()
                    .is_some_and(|d| d.is_text() && d.is_simple_text());
                let first_descendant_does_not_overflow = previous_plus_descendant_length <= offset;
                if first_descendant_is_simple_text || first_descendant_does_not_overflow {
                    unwrap_node(&node)?;
                }
            }
        } else if is_slot_value_leaf {
            // Not wrapped, but its text is part of the budget.
            accumulated_length += node.get_text_content_size();
        } else if needs_overflow_parent {
            let previous_accumulated_length = accumulated_length;
            accumulated_length += node.get_text_content_size();
            if accumulated_length > offset
                && !node.get_parent().is_some_and(|p| is_overflow_node(&p))
            {
                let previous_selection = get_selection();
                // Simple text is split at the limit.
                let overflow_node = if previous_accumulated_length < offset
                    && node.is_text()
                    && node.is_simple_text()
                {
                    let parts =
                        node.split_text(&[(offset - previous_accumulated_length) as i64])?;
                    wrap_node(&parts[1])?
                } else {
                    wrap_node(&node)?
                };
                if let Some(previous_selection) = previous_selection {
                    set_selection(Some(previous_selection));
                }
                merge_previous(&overflow_node)?;
                if let Some(next_node) = overflow_node.get_next_sibling()
                    && is_overflow_node(&next_node)
                {
                    merge_previous(&next_node)?;
                }
            }
        }
    }
    Ok(())
}

/// `$wrapNode(node)`.
fn wrap_node(node: &NodeRef) -> Result<NodeRef, LexicalError> {
    let overflow_node = create_overflow_node();
    node.replace(&overflow_node, false)?;
    overflow_node.append(std::slice::from_ref(node))?;
    Ok(overflow_node)
}

/// `$mergePrevious(overflowNode)`: moves the children of the
/// `OverflowNode` before `overflow_node` into it, with the selection in
/// either, and removes that one. Nothing happens when the node before isn't
/// an `OverflowNode`.
pub fn merge_previous(overflow_node: &NodeRef) -> Result<(), LexicalError> {
    debug_assert!(overflow_node.get_latest().is::<OverflowNode>());
    let Some(previous_node) = overflow_node
        .get_previous_sibling()
        .filter(is_overflow_node)
    else {
        return Ok(());
    };
    let first_child = overflow_node.get_first_child();
    let previous_node_children = previous_node.get_children();
    let previous_node_children_length = previous_node_children.len();
    match first_child {
        None => overflow_node.append(&previous_node_children)?,
        Some(first_child) => {
            for child in &previous_node_children {
                first_child.insert_before(child, true)?;
            }
        }
    }
    if let Some(selection) = get_selection().filter(|s| s.is_range()) {
        for which in [Which::Anchor, Which::Focus] {
            let point = selection.point(which).expect("a range selection");
            let point_node = point.get_node()?;
            if point_node.get_key() == previous_node.get_key() {
                selection.point_set(
                    which,
                    overflow_node.get_key(),
                    point.offset,
                    PointType::Element,
                )?;
            } else if point_node.get_key() == overflow_node.get_key() {
                selection.point_set(
                    which,
                    overflow_node.get_key(),
                    previous_node_children_length + point.offset,
                    PointType::Element,
                )?;
            }
        }
    }
    previous_node.remove(false)
}
