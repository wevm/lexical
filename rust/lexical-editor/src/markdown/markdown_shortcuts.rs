//! `MarkdownShortcuts.ts`: `registerMarkdownShortcuts`, which turns
//! markdown typed into the editor into rich text as it is typed (`# `
//! makes a heading, `**bold**` bold text), and on Enter for the
//! transformers that allow it (a code fence).

use super::import_text_transformers::can_contain_transformable_markdown;
use super::markdown_transformers::{
    ElementTransformer, MultilineElementTransformer, TextFormatTransformer, TextMatchTransformer,
    Transformer,
};
use super::utils::{
    character_at, character_before, index_by, is_line_break_node, is_punctuation_or_space, string,
    transformers_by_type, unit_at, units,
};
use crate::code::is_code_node;
use crate::constants::TEXT_TYPE_TO_FORMAT;
use crate::lexical_selection::{Point, PointType, Which};
use crate::selection_insert_nodes::is_block_container;
use crate::updates::get_active_editor;
use crate::{
    COMMAND_PRIORITY_LOW, COMPOSITION_END_TAG, Editor, HISTORIC_TAG, HISTORY_PUSH_TAG,
    KEY_ENTER_COMMAND, LexicalError, NodeRef, Unregister, UpdateOptions, create_range_selection,
    get_node_by_key, get_selection, set_selection,
};
use std::collections::HashSet;
use std::rc::Rc;

/// `COLLABORATION_TAG` (`LexicalUpdateTags.ts`): the update came from a
/// collaborator.
const COLLABORATION_TAG: &str = "collaboration";

const SPACE: u32 = b' ' as u32;

/// `runElementTransformers(parentNode, anchorNode, anchorOffset,
/// elementTransformers, triggerOnEnter)`.
fn run_element_transformers(
    parent_node: &NodeRef,
    anchor_node: &NodeRef,
    anchor_offset: usize,
    element_transformers: &[Rc<ElementTransformer>],
    trigger_on_enter: bool,
) -> Result<bool, LexicalError> {
    if !at_line_start(parent_node, anchor_node) {
        return Ok(false);
    }
    let text_content = anchor_node.get_text_content();
    // Only a caret right after a space can end a block-level trigger.
    if !trigger_on_enter
        && unit_at(&units(&text_content), anchor_offset as isize - 1) != Some(SPACE)
    {
        return Ok(false);
    }
    for transformer in element_transformers {
        let Some(m) = transformer.reg_exp.match_first(&text_content) else {
            continue;
        };
        let expected_match_length = if trigger_on_enter || m.text().ends_with(' ') {
            anchor_offset as isize
        } else {
            anchor_offset as isize - 1
        };
        if m.match_len() as isize == expected_match_length {
            let siblings = split_at_caret(anchor_node, anchor_offset)?;
            if (transformer.replace)(parent_node, &siblings.rest, &m, false)? {
                siblings.leading.remove(false)?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Whether `anchor_node` starts `parent_node`, a block in a block container,
/// where a block-level trigger can be.
fn at_line_start(parent_node: &NodeRef, anchor_node: &NodeRef) -> bool {
    parent_node
        .get_parent()
        .is_some_and(|grand_parent| is_block_container(&grand_parent))
        && parent_node
            .get_first_child()
            .is_some_and(|first| first.get_key() == anchor_node.get_key())
}

/// The trigger the caret ends and what follows it, after `anchorNode.splitText(anchorOffset)`.
struct SplitAtCaret {
    leading: NodeRef,
    /// The remainder of the text node, if any, then its next siblings.
    rest: Vec<NodeRef>,
}

fn split_at_caret(
    anchor_node: &NodeRef,
    anchor_offset: usize,
) -> Result<SplitAtCaret, LexicalError> {
    let next_siblings = anchor_node.get_next_siblings();
    let mut parts = anchor_node.split_text(&[anchor_offset as i64])?.into_iter();
    let leading = parts.next().expect("a part");
    let mut rest: Vec<NodeRef> = parts.next().into_iter().collect();
    rest.extend(next_siblings);
    Ok(SplitAtCaret { leading, rest })
}

/// `runMultilineElementTransformers(parentNode, anchorNode, anchorOffset,
/// elementTransformers, triggerOnEnter)`: only those whose end is optional
/// (or that have none) run as shortcuts.
fn run_multiline_element_transformers(
    parent_node: &NodeRef,
    anchor_node: &NodeRef,
    anchor_offset: usize,
    element_transformers: &[Rc<MultilineElementTransformer>],
    trigger_on_enter: bool,
) -> Result<bool, LexicalError> {
    if !at_line_start(parent_node, anchor_node) {
        return Ok(false);
    }
    let text_content = anchor_node.get_text_content();
    if !trigger_on_enter
        && unit_at(&units(&text_content), anchor_offset as isize - 1) != Some(SPACE)
    {
        return Ok(false);
    }
    for transformer in element_transformers {
        if transformer
            .reg_exp_end
            .as_ref()
            .is_some_and(|end| !end.optional)
        {
            continue;
        }
        let Some(m) = transformer.reg_exp_start.match_first(&text_content) else {
            continue;
        };
        let match_length = if trigger_on_enter || m.text().ends_with(' ') {
            anchor_offset as isize
        } else {
            anchor_offset as isize - 1
        };
        if m.match_len() as isize != match_length {
            continue;
        }
        let siblings = split_at_caret(anchor_node, anchor_offset)?;
        if (transformer.replace)(parent_node, Some(&siblings.rest), &m, None, None, false)? {
            siblings.leading.remove(false)?;
            return Ok(true);
        }
    }
    Ok(false)
}

/// `runTextMatchTransformers(anchorNode, anchorOffset,
/// transformersByTrigger)`.
fn run_text_match_transformers(
    anchor_node: &NodeRef,
    anchor_offset: usize,
    transformers_by_trigger: &[(String, Vec<Rc<TextMatchTransformer>>)],
) -> Result<bool, LexicalError> {
    let text = units(&anchor_node.get_text_content());
    let Some(last_char) = unit_at(&text, anchor_offset as isize - 1) else {
        return Ok(false);
    };
    let last_char = string(&[last_char as u16]);
    let Some((_, transformers)) = transformers_by_trigger
        .iter()
        .find(|(k, _)| *k == last_char)
    else {
        return Ok(false);
    };
    // Typing in the middle of the text matches up to the caret.
    let text_content = string(&text[..anchor_offset.min(text.len())]);
    for transformer in transformers {
        let Some(replace) = &transformer.replace else {
            continue;
        };
        let Some(m) = transformer.reg_exp.match_first(&text_content) else {
            continue;
        };
        let start_index = m.index;
        let end_index = start_index + m.match_len();
        let replace_node = if start_index == 0 {
            anchor_node
                .split_text(&[end_index as i64])?
                .into_iter()
                .next()
        } else {
            anchor_node
                .split_text(&[start_index as i64, end_index as i64])?
                .into_iter()
                .nth(1)
        }
        .expect("a part");
        replace_node.select_next(Some(0), Some(0))?;
        replace(&replace_node, &m)?;
        return Ok(true);
    }
    Ok(false)
}

/// `$runTextFormatTransformers(anchorNode, anchorOffset,
/// textFormatTransformers)`: a closing tag typed at the caret formats the
/// text since its opening tag, in this text node or an earlier one of the
/// line.
fn run_text_format_transformers(
    anchor_node: &NodeRef,
    anchor_offset: usize,
    text_format_transformers: &[(String, Vec<Rc<TextFormatTransformer>>)],
) -> Result<bool, LexicalError> {
    let text_content = units(&anchor_node.get_text_content());
    let close_tag_end_index = anchor_offset as isize - 1;
    let Some(close_char) = unit_at(&text_content, close_tag_end_index) else {
        return Ok(false);
    };
    // A quick check that the caret may end an inline format.
    let close_key = string(&[close_char as u16]);
    let Some((_, matchers)) = text_format_transformers
        .iter()
        .find(|(k, _)| *k == close_key)
    else {
        return Ok(false);
    };

    for matcher in matchers {
        let tag = units(&matcher.tag);
        let tag_length = tag.len() as isize;
        let close_tag_start_index = close_tag_end_index - tag_length + 1;

        // A tag longer than one character must match the text before it.
        if tag_length > 1 && !is_equal_sub_string(&text_content, close_tag_start_index, &tag) {
            continue;
        }

        // A space before the closing tag cancels it.
        if unit_at(&text_content, close_tag_start_index - 1) == Some(SPACE) {
            continue;
        }

        // Some tags can't be used inside a word, so a space, punctuation or
        // the end of the line must follow them.
        let after_close_tag_char = character_at(&text_content, close_tag_end_index + 1);
        if matcher.intraword == Some(false)
            && after_close_tag_char.is_some()
            && !is_punctuation_or_space(after_close_tag_char)
        {
            continue;
        }

        let close_node = anchor_node.clone();
        let mut open_node = close_node.clone();
        let mut open_tag_start_index =
            get_open_tag_start_index(&text_content, close_tag_start_index, &tag);

        // Look for the opening tag in the text nodes before the closing
        // one's, up to a line break.
        let mut sibling = open_node.clone();
        while open_tag_start_index < 0 {
            let Some(previous) = sibling.get_previous_sibling() else {
                break;
            };
            sibling = previous;
            if is_line_break_node(&sibling) {
                break;
            }
            if sibling.is_text() {
                if sibling.has_format("code") {
                    continue;
                }
                let sibling_text_content = units(&sibling.get_text_content());
                open_node = sibling.clone();
                open_tag_start_index = get_open_tag_start_index(
                    &sibling_text_content,
                    sibling_text_content.len() as isize,
                    &tag,
                );
            }
        }

        // No opening tag.
        if open_tag_start_index < 0 {
            continue;
        }

        let same_node = open_node.get_key() == close_node.get_key();
        // Nothing between the tags.
        if same_node && open_tag_start_index + tag_length == close_tag_start_index {
            continue;
        }

        // A longer tag of the same character (`***` for `**`).
        let prev_open_node_text = units(&open_node.get_text_content());
        if open_tag_start_index > 0
            && unit_at(&prev_open_node_text, open_tag_start_index - 1) == Some(close_char)
        {
            continue;
        }

        // Some tags can't be used inside a word, so a space, punctuation or
        // the start of the line must come before them.
        let before_open_tag_char = character_before(&prev_open_node_text, open_tag_start_index);
        if matcher.intraword == Some(false)
            && before_open_tag_char.is_some()
            && !is_punctuation_or_space(before_open_tag_char)
        {
            continue;
        }

        // CommonMark: code spans take precedence over other inline formats.
        if !matcher.format.contains(&"code")
            && is_inside_unclosed_code_span(&open_node, open_tag_start_index as usize)
        {
            continue;
        }

        // Remove the tags, the closing one first so the opening one's offset
        // stays.
        let prev_close_node_text = units(&close_node.get_text_content());
        let close_tag_start = close_tag_start_index as usize;
        let close_tag_end = close_tag_end_index as usize;
        let mut close_node_text = prev_close_node_text[..close_tag_start].to_vec();
        close_node_text.extend_from_slice(
            &prev_close_node_text[(close_tag_end + 1).min(prev_close_node_text.len())..],
        );
        close_node.set_text_content(&string(&close_node_text));
        let open_node_text = if same_node {
            close_node_text
        } else {
            prev_open_node_text
        };
        let open_tag_start = open_tag_start_index as usize;
        let mut new_open_node_text = open_node_text[..open_tag_start].to_vec();
        new_open_node_text.extend_from_slice(
            &open_node_text[(open_tag_start + tag.len()).min(open_node_text.len())..],
        );
        open_node.set_text_content(&string(&new_open_node_text));

        let selection = get_selection();
        let next_selection = create_range_selection();
        set_selection(Some(next_selection.clone()));
        // The offset less the removed tags.
        let new_offset = close_tag_end_index - tag_length * if same_node { 2 } else { 1 } + 1;
        next_selection.point_set(
            Which::Anchor,
            open_node.get_key(),
            open_tag_start,
            PointType::Text,
        )?;
        next_selection.point_set(
            Which::Focus,
            close_node.get_key(),
            new_offset as usize,
            PointType::Text,
        )?;

        // Format the text between the tags.
        for format in &matcher.format {
            let flag = TEXT_TYPE_TO_FORMAT
                .iter()
                .find(|(name, _)| name == format)
                .map(|(_, flag)| f64::from(*flag));
            next_selection.format_text(format, flag)?;
        }

        // Collapse the selection to the focus.
        let focus = next_selection
            .point(Which::Focus)
            .expect("a range selection");
        next_selection.point_set(Which::Anchor, &focus.key, focus.offset, focus.point_type)?;

        // Typing on doesn't continue the formats.
        for format in &matcher.format {
            if next_selection.has_format(format) {
                next_selection.toggle_format(format);
            }
        }

        if let Some(range) = selection.and_then(|s| s.range()) {
            next_selection.set_format(range.format);
        }
        return Ok(true);
    }
    Ok(false)
}

/// `$isInsideUnclosedCodeSpan(node, offset)`: whether an odd number of
/// backticks comes before `offset`, so it is inside a code span not yet
/// closed (CommonMark gives code spans precedence over other formats).
fn is_inside_unclosed_code_span(node: &NodeRef, offset: usize) -> bool {
    let text = units(&node.get_text_content());
    let backtick_count = text
        .iter()
        .take(offset)
        .filter(|&&u| u == u16::from(b'`'))
        .count();
    backtick_count % 2 != 0
}

/// `getOpenTagStartIndex(string, maxIndex, tag)`: where the last opening
/// `tag` before `max_index` starts, one not followed by a space, or -1.
fn get_open_tag_start_index(string: &[u16], max_index: isize, tag: &[u16]) -> isize {
    let tag_length = tag.len() as isize;
    let mut i = max_index;
    while i >= tag_length {
        let start_index = i - tag_length;
        // A space after the opening tag cancels it.
        if is_equal_sub_string(string, start_index, tag)
            && unit_at(string, start_index + tag_length) != Some(SPACE)
        {
            return start_index;
        }
        i -= 1;
    }
    -1
}

/// `$getOffsetInParent(point)`: the point's offset in characters from the
/// start of its node's parent, so that two offsets stay comparable when
/// transforms split or merge the text around the caret (#5366).
fn get_offset_in_parent(point: &Point) -> Result<usize, LexicalError> {
    let node = point.get_node()?;
    let mut offset = 0;
    if node.is_text() {
        offset = point.offset;
    } else if node.is_element() {
        for child in node.get_children().iter().take(point.offset) {
            offset += child.get_text_content_size();
        }
    }
    let mut sibling = node.get_previous_sibling();
    while let Some(s) = sibling {
        offset += s.get_text_content_size();
        sibling = s.get_previous_sibling();
    }
    Ok(offset)
}

/// `isEqualSubString(stringA, aStart, stringB, 0, stringB.length)`.
fn is_equal_sub_string(string_a: &[u16], a_start: isize, string_b: &[u16]) -> bool {
    string_b
        .iter()
        .enumerate()
        .all(|(i, &unit)| unit_at(string_a, a_start + i as isize) == Some(u32::from(unit)))
}

/// What the update listener and the Enter handler run, by type.
struct Shortcuts {
    element: Vec<Rc<ElementTransformer>>,
    element_for_enter: Vec<Rc<ElementTransformer>>,
    multiline_element: Vec<Rc<MultilineElementTransformer>>,
    text_format_by_trigger: Vec<(String, Vec<Rc<TextFormatTransformer>>)>,
    text_match_by_trigger: Vec<(String, Vec<Rc<TextMatchTransformer>>)>,
    composition_end_trigger_chars: HashSet<String>,
}

impl Shortcuts {
    /// `$transform(parentNode, anchorNode, anchorOffset)`.
    fn transform(
        &self,
        parent_node: &NodeRef,
        anchor_node: &NodeRef,
        anchor_offset: usize,
    ) -> Result<bool, LexicalError> {
        Ok(run_element_transformers(
            parent_node,
            anchor_node,
            anchor_offset,
            &self.element,
            false,
        )? || run_multiline_element_transformers(
            parent_node,
            anchor_node,
            anchor_offset,
            &self.multiline_element,
            false,
        )? || run_text_match_transformers(
            anchor_node,
            anchor_offset,
            &self.text_match_by_trigger,
        )? || run_text_format_transformers(
            anchor_node,
            anchor_offset,
            &self.text_format_by_trigger,
        )?)
    }
}

/// The last UTF-16 code unit of `text`, as a string (JS's
/// `text[text.length - 1]` and `text.slice(-1)`).
fn last_unit(text: &str) -> Option<String> {
    units(text).last().map(|&u| string(&[u]))
}

/// `registerMarkdownShortcuts(editor, transformers)`: the update listener
/// that runs the transformers as markdown is typed, and the
/// `KEY_ENTER_COMMAND` handler (at `COMMAND_PRIORITY_LOW`) that runs the
/// multiline element transformers and the element transformers with
/// `trigger_on_enter` on Enter at the end of a line. Fails, as JS's
/// invariant, when the editor lacks a node type a transformer depends on.
///
/// The editor is never composing here (`editor.isComposing()`): an app
/// keeps an input method's composition outside Lexical until it commits
/// (rust/DESIGN.md), and the commit's `composition-end` tag works as in JS.
pub fn register_markdown_shortcuts(
    editor: &Editor,
    transformers: &[Transformer],
) -> Result<Unregister, LexicalError> {
    let by_type = transformers_by_type(transformers);
    let mut composition_end_trigger_chars: HashSet<String> = HashSet::from([" ".to_owned()]);
    for t in &by_type.text_format {
        composition_end_trigger_chars.extend(last_unit(&t.tag));
    }
    for t in &by_type.text_match {
        composition_end_trigger_chars.extend(t.trigger.clone());
    }
    for transformer in transformers {
        if matches!(
            transformer,
            Transformer::Element(_) | Transformer::TextMatch(_) | Transformer::MultilineElement(_)
        ) {
            for node in transformer.dependencies() {
                crate::error::invariant!(
                    editor.registered_nodes().contains_key(node),
                    "MarkdownShortcuts: missing dependency {node} for transformer. Ensure node dependency is included in editor initial config."
                );
            }
        }
    }
    let shortcuts = Rc::new(Shortcuts {
        element_for_enter: by_type
            .element
            .iter()
            .filter(|t| t.trigger_on_enter)
            .cloned()
            .collect(),
        text_format_by_trigger: index_by(&by_type.text_format, |t| last_unit(&t.tag)),
        text_match_by_trigger: index_by(&by_type.text_match, |t| t.trigger.clone()),
        element: by_type.element,
        multiline_element: by_type.multiline_element,
        composition_end_trigger_chars,
    });

    let listener_shortcuts = shortcuts.clone();
    let listener_editor = editor.clone();
    let unregister_listener = editor.register_update_listener(move |payload| {
        let has_tag = |tag: &str| payload.tags.iter().any(|t| t == tag);
        // Collaboration and undo/redo bring changes already transformed.
        if has_tag(COLLABORATION_TAG) || has_tag(HISTORIC_TAG) {
            return;
        }
        // A composition's commit lands without moving the selection (the
        // composition did), and may jump the anchor by more than one, so
        // the typed-character checks are skipped for it.
        let is_composition_end = has_tag(COMPOSITION_END_TAG);
        let selection = payload
            .editor_state
            .read(get_selection)
            .and_then(|s| s.range());
        let prev_selection = payload
            .prev_editor_state
            .read(get_selection)
            .and_then(|s| s.range());
        // Only as the user types: a collapsed range selection that moved.
        let (Some(selection), Some(prev_selection)) = (selection, prev_selection) else {
            return;
        };
        if !selection.is_collapsed() || (selection.is(&prev_selection) && !is_composition_end) {
            return;
        }
        let anchor_key = selection.anchor.key.clone();
        let anchor_offset = selection.anchor.offset;
        let is_text = payload
            .editor_state
            .read(|| get_node_by_key(&anchor_key).is_some_and(|n| n.is_text()));
        if !is_text || !payload.dirty_leaves.contains(&anchor_key) {
            return;
        }
        if !is_composition_end && anchor_offset != 1 {
            let offset = payload
                .editor_state
                .read(|| get_offset_in_parent(&selection.anchor));
            let prev_offset = payload
                .prev_editor_state
                .read(|| get_offset_in_parent(&prev_selection.anchor));
            if let (Ok(offset), Ok(prev_offset)) = (offset, prev_offset)
                && offset > prev_offset + 1
            {
                return;
            }
        }
        if is_composition_end {
            let close_char = payload.editor_state.read(|| {
                let text = units(&get_node_by_key(&anchor_key)?.get_text_content());
                unit_at(&text, anchor_offset as isize - 1).map(|u| string(&[u as u16]))
            });
            if !close_char.is_some_and(|c| {
                listener_shortcuts
                    .composition_end_trigger_chars
                    .contains(&c)
            }) {
                return;
            }
        }
        let shortcuts = listener_shortcuts.clone();
        // Queued after this commit, as JS's `editor.update` in a listener;
        // a failure is the update's own, discarded with it.
        let _ = listener_editor.update(
            move || {
                let anchor_node = get_node_by_key(&anchor_key);
                if !can_contain_transformable_markdown(anchor_node.as_ref()) {
                    return Ok(());
                }
                let anchor_node = anchor_node.expect("a text node");
                let Some(parent_node) = anchor_node.get_parent() else {
                    return Ok(());
                };
                if is_code_node(&parent_node) {
                    return Ok(());
                }
                if shortcuts.transform(&parent_node, &anchor_node, anchor_offset)?
                    && let Some(editor) = get_active_editor()
                {
                    editor.add_tags(&[HISTORY_PUSH_TAG.to_owned()]);
                }
                Ok(())
            },
            UpdateOptions::default(),
        );
    });

    let unregister_enter = editor.register_command(
        &KEY_ENTER_COMMAND,
        move |event, _| {
            if event.as_ref().is_some_and(|e| e.shift_key) {
                return Ok(false);
            }
            let Some(selection) = get_selection().and_then(|s| s.range()) else {
                return Ok(false);
            };
            if !selection.is_collapsed() {
                return Ok(false);
            }
            let anchor_offset = selection.anchor.offset;
            let anchor_node = selection.anchor.get_node()?;
            if !can_contain_transformable_markdown(Some(&anchor_node)) {
                return Ok(false);
            }
            let Some(parent_node) = anchor_node.get_parent() else {
                return Ok(false);
            };
            if is_code_node(&parent_node) {
                return Ok(false);
            }
            let text_content = anchor_node.get_text_content();
            if anchor_offset != crate::utf16::len(&text_content) {
                return Ok(false);
            }
            if run_multiline_element_transformers(
                &parent_node,
                &anchor_node,
                anchor_offset,
                &shortcuts.multiline_element,
                true,
            )? || run_element_transformers(
                &parent_node,
                &anchor_node,
                anchor_offset,
                &shortcuts.element_for_enter,
                true,
            )? {
                if let Some(event) = event {
                    event.prevent_default();
                }
                return Ok(true);
            }
            Ok(false)
        },
        COMMAND_PRIORITY_LOW,
    );
    Ok(Box::new(move || {
        unregister_listener();
        unregister_enter();
    }))
}
