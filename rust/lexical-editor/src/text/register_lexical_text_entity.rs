//! `registerLexicalTextEntity.ts`: `registerLexicalTextEntity` and the
//! `EntityMatch` its matcher returns.
//!
//! NodeState isn't ported (rust/PARITY.md), so what upstream does only to
//! carry it between text and entities is left out: `$carryNodeState`, the
//! NodeState half of `$wouldMerge` (`haveEquivalentNodeState`), and the
//! bookkeeping that only decides whether to carry it (`splitOffInUpdate`,
//! `demotedInUpdate`, `$isInsertedText` and the offsets they take). Nor is
//! the text edit log, so `INTERNAL_$recordTextMove` just runs its callback
//! and `INTERNAL_$recordTextRetype` is not called.

use crate::editor::EditorInner;
use crate::editor_state::EditorStateInner;
use crate::error::invariant;
use crate::lexical_selection::{PointType, SelectionRef, Which, get_selection};
use crate::node::{NodeKey, NodeRef};
use crate::nodes::text::create_text_node;
use crate::updates::internal_get_active_editor_state;
use crate::{Editor, EditorState, LexicalError, Unregister, utf16};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

/// `EntityMatch`: where a matcher found an entity in a text, in UTF-16
/// code units, from `start` up to `end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityMatch {
    pub end: usize,
    pub start: usize,
}

// How many matches in a text node are passed over to check the node after it,
// where its transform returns before the loop that checks that node.
const MAX_MATCHES_PASSED: usize = 8;

// How many times one update turns the same token or segmented entity back into
// text before it leaves it as an entity.
const MAX_TURNED_INTO_TEXT: usize = 2;

/// `$wouldMerge(a, b)`: whether `a` and `b` would merge as text, the same
/// format, style and detail (and NodeState, which isn't ported).
fn would_merge(a: &NodeRef, b: &NodeRef) -> bool {
    let (a, b) = (a.text_data(), b.text_data());
    a.format == b.format && a.style == b.style && a.detail == b.detail
}

/// `$moveSelectionIntoEntity(node, entity, count)`: before `entity` takes
/// the first `count` characters of `node`, the text node after it, moves a
/// selection point in those characters (or right after them) into the
/// entity, and one in the rest of `node` back by `count`.
fn move_selection_into_entity(
    node: &NodeRef,
    entity: &NodeRef,
    count: usize,
) -> Result<(), LexicalError> {
    let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
        return Ok(());
    };
    let key = node.get_key().to_owned();
    let entity_size = entity.get_text_content_size();
    for which in [Which::Anchor, Which::Focus] {
        let Some(point) = selection.point(which) else {
            continue;
        };
        if point.key == key && point.point_type == PointType::Text {
            if point.offset <= count {
                selection.point_set(
                    which,
                    entity.get_key(),
                    entity_size + point.offset,
                    PointType::Text,
                )?;
            } else {
                selection.point_set(which, &key, point.offset - count, PointType::Text)?;
            }
        }
    }
    Ok(())
}

/// What upstream keeps in a `WeakMap` by pending editor state: the value
/// for the state of the update running. One state is pending at a time, so
/// it holds the one for the last state asked about, and starts again with
/// another.
struct InUpdate<T>(RefCell<Option<(Weak<EditorStateInner>, T)>>);

impl<T: Default> InUpdate<T> {
    fn new() -> Self {
        Self(RefCell::new(None))
    }

    fn with<R>(&self, editor_state: &EditorState, f: impl FnOnce(&mut T) -> R) -> R {
        let mut slot = self.0.borrow_mut();
        let current = slot.as_ref().is_some_and(|(state, _)| {
            state
                .upgrade()
                .is_some_and(|state| Rc::ptr_eq(&state, &editor_state.0))
        });
        if !current {
            *slot = Some((Rc::downgrade(&editor_state.0), T::default()));
        }
        f(&mut slot.as_mut().expect("just set").1)
    }
}

/// The matcher a registration finds entities with.
type GetMatch = Box<dyn Fn(&str) -> Option<EntityMatch>>;

/// One `registerLexicalTextEntity` call: its arguments and the closures it
/// makes, as methods.
struct TextEntity {
    editor: Weak<EditorInner>,
    get_match: GetMatch,
    target_type: &'static str,
    create_node: Box<dyn Fn(&NodeRef) -> NodeRef>,
    // The keys of the text nodes the entity transform marked dirty, by pending
    // editor state (one update, or the updates batched into one commit).
    marked_in_update: InUpdate<HashSet<NodeKey>>,
    // How many times each token or segmented entity was turned back into text,
    // by pending editor state (one update, or the updates batched into one
    // commit) and its parent, place and text.
    turned_into_text_in_update: InUpdate<HashMap<String, usize>>,
}

/// `$isTextNode(node) && node.isTextEntity()`.
fn is_text_entity(node: &NodeRef) -> bool {
    node.is_text() && node.is_text_entity()
}

impl TextEntity {
    /// `INTERNAL_getActiveEditorState() || editor.getEditorState()`.
    fn update_state(&self) -> EditorState {
        internal_get_active_editor_state().unwrap_or_else(|| {
            Editor(self.editor.upgrade().expect("the editor")).get_editor_state()
        })
    }

    /// `isTargetNode(node)`: whether `node` is of the target type.
    fn is_target_node(&self, node: &NodeRef) -> bool {
        node.get_type() == self.target_type
    }

    /// `$replaceWithSimpleText(node)`.
    fn replace_with_simple_text(&self, node: &NodeRef) -> Result<(), LexicalError> {
        let data = node.text_data();
        let text_node = create_text_node(&node.get_text_content());
        text_node.set_format(data.format);
        text_node.set_style(&data.style);
        text_node.set_detail(data.detail)?;
        node.replace(&text_node, false)?;
        Ok(())
    }

    /// `$wasTurnedIntoText(entity, why)`: whether a token or segmented
    /// entity with the same parent, place and text as `entity` was turned
    /// back into text as many times as it may be in one update; counts this
    /// time if not.
    fn was_turned_into_text(&self, entity: &NodeRef, why: &str) -> bool {
        // Told by the first node before it that is no text entity, which the
        // transforms leave in place while they make the entity and turn it back
        // (text that merges with it keeps its key), and the size of the text
        // entities between (which they may make again too).
        let mut before = entity.get_previous_sibling();
        let mut entities_before = 0;
        while let Some(node) = before.clone().filter(is_text_entity) {
            entities_before += node.get_text_content_size();
            before = node.get_previous_sibling();
        }
        let at = format!(
            "{}+{entities_before}",
            before.as_ref().map_or("", |n| n.get_key())
        );
        let parent = entity.get_parent();
        let id = format!(
            "{why}{}:{at}:{}",
            parent.as_ref().map_or("", |p| p.get_key()),
            entity.get_text_content()
        );
        let update_state = self.update_state();
        self.turned_into_text_in_update
            .with(&update_state, |turned| {
                let count = turned.get(&id).copied().unwrap_or(0);
                if count >= MAX_TURNED_INTO_TEXT {
                    return true;
                }
                turned.insert(id, count + 1);
                false
            })
    }

    /// `getMode(node)`: `__mode`.
    fn get_mode(node: &NodeRef) -> u8 {
        node.text_data().mode
    }

    /// `getFirstMatchAfter(text, textAfter)`: whether the first match of
    /// `textAfter` after `text` starts where `textAfter` does, passing over
    /// the matches of `text` as the loop of the text transform does (it
    /// leaves those as text when the transform returns before it), and
    /// telling a match that ends where `textAfter` starts (which touches it)
    /// from one that starts there. `None` when `text` has more matches than
    /// are worth passing over.
    fn get_first_match_after(&self, text: &str, text_after: &str) -> Option<bool> {
        let combined_text = format!("{text}{text_after}");
        let text_length = utf16::len(text);
        let mut offset = 0;
        for _ in 0..MAX_MATCHES_PASSED {
            let Some(found) = (self.get_match)(&utf16::slice_from(&combined_text, offset)) else {
                return Some(false);
            };
            let start = offset + found.start;
            let end = offset + found.end;
            if end <= start {
                return None;
            }
            if end >= text_length {
                return Some(start == text_length);
            }
            offset = end;
        }
        None
    }

    /// `isNextTextEntity(node)`: whether the node after `node` is a text
    /// entity (an entity right after another is handled by the entity's own
    /// transform).
    fn is_next_text_entity(node: &NodeRef) -> bool {
        node.get_next_sibling().is_some_and(|n| is_text_entity(&n))
    }

    /// `isTextAfterOutsideSource(entity)`: whether a text node after
    /// `entity` is not text it was split from: one in another format, style
    /// or detail, or not simple text.
    fn is_text_after_outside_source(&self, entity: &NodeRef) -> bool {
        let Some(after) = entity.get_next_sibling().filter(NodeRef::is_text) else {
            return false;
        };
        let (a, e) = (after.text_data(), entity.text_data());
        !(after.is_simple_text()
            && !self.is_target_node(&after)
            && a.format == e.format
            && a.style == e.style
            && a.detail == e.detail)
    }

    /// `isEntityAfter(textBefore, entity, ifUnsure)`: whether `entity`,
    /// right after a simple text node with `text_before`, is an entity the
    /// transforms keep: the first match from `text_before` on starts where
    /// it does, with the text after it in its source node (the simple text
    /// node after it in the same format, style and detail, which it was
    /// split from), as the check that made it an entity found it; and that
    /// match still holds with the text of any other text node after it, as
    /// the check that keeps a match at the end of a node from becoming an
    /// entity found it. When `text_before` has too many matches to tell,
    /// `if_unsure` is returned.
    fn is_entity_after(&self, text_before: &str, entity: &NodeRef, if_unsure: bool) -> bool {
        let holds = |result: Option<bool>| result.unwrap_or(if_unsure);
        let entity_text = entity.get_text_content();
        let Some(after) = entity.get_next_sibling().filter(NodeRef::is_text) else {
            return holds(self.get_first_match_after(text_before, &entity_text));
        };
        let is_suffix = !self.is_text_after_outside_source(entity);
        let with_after = self.get_first_match_after(
            text_before,
            &format!("{entity_text}{}", after.get_text_content()),
        );
        if is_suffix {
            return holds(with_after);
        }
        holds(with_after) && holds(self.get_first_match_after(text_before, &entity_text))
    }

    /// `$checkNextSibling(node, text)`: checks the entity after a simple
    /// text node whose transform returned before its loop, which checks that
    /// node: it is turned back into text unless the first match from this
    /// node's text on, with the text after the entity, starts where the
    /// entity does, as it must for a match at the start of a node to be made
    /// an entity.
    fn check_next_sibling(&self, node: &NodeRef, text: &str) -> Result<(), LexicalError> {
        if let Some(next_sibling) = node.get_next_sibling().filter(|n| self.is_target_node(n)) {
            let entity_text = next_sibling.get_text_content();
            let entity_match = (self.get_match)(&entity_text);
            if entity_match.is_none_or(|m| m.start != 0 || m.end != utf16::len(&entity_text)) {
                // Leave an entity that its own transform is still splitting or
                // turning back into text to that transform.
                return Ok(());
            }
            if !self.is_entity_after(text, &next_sibling, true) {
                self.replace_with_simple_text(&next_sibling)?;
            }
        }
        Ok(())
    }

    /// `$textNodeTransform(node)`.
    fn text_node_transform(&self, node: &NodeRef) -> Result<(), LexicalError> {
        if !node.is_simple_text() {
            return Ok(());
        }

        let mut prev_sibling = node.get_previous_sibling();
        let mut text = node.get_text_content();
        let mut current_node = node.clone();
        // The first match of the previous text node's text and this node's.
        let mut first_match: Option<EntityMatch> = None;

        if let Some(prev) = prev_sibling.clone().filter(NodeRef::is_text) {
            let previous_text = prev.get_text_content();
            let previous_length = utf16::len(&previous_text);
            let combined_text = format!("{previous_text}{text}");
            let prev_match = (self.get_match)(&combined_text);
            first_match = prev_match;

            if self.is_target_node(&prev) {
                match prev_match {
                    Some(prev_match)
                        if Self::get_mode(&prev) == 0
                            // A token or segmented entity followed by text
                            // is turned back into text, unless the same
                            // entity was already turned back into text twice
                            // for this pending editor state: the transforms
                            // would only make it again and turn it back
                            // forever. It is kept then (below).
                            || (prev_match.start == 0
                                && prev_match.end >= previous_length
                                && self.was_turned_into_text(&prev, "")) =>
                    {
                        // Whether the entity takes the first `diff`
                        // characters of this node
                        let mut grows = prev_match.end > previous_length;
                        let text_before = prev.get_previous_sibling();
                        if grows && let Some(text_before) = text_before.filter(NodeRef::is_text) {
                            if text_before.is_text_entity() {
                                // Two entities next to each other: the
                                // entities' own transforms turn them back
                                // into text (or keep them as they are); this
                                // one does not take this node's text first.
                                grows = false;
                            } else if text_before.is_simple_text() {
                                let before = text_before.get_text_content();
                                if !self.is_entity_after(&before, &prev, true) {
                                    // An entity that the transforms do not
                                    // keep after the text before it is turned
                                    // back into text rather than grown over
                                    // this node first.
                                    self.replace_with_simple_text(&prev)?;

                                    return Ok(());
                                }
                                if !self.is_entity_after(&before, &prev, false) {
                                    // Not sure (too many matches in the text
                                    // before it to tell): it stays as it is,
                                    // without taking this node's text.
                                    grows = false;
                                }
                            }
                        }
                        if grows
                            // A token or segmented entity kept above takes
                            // the text its match extends into, as an entity
                            // in normal mode does, only when that text would
                            // merge with it (the same format, style and
                            // detail); text with other format stays apart.
                            && (Self::get_mode(&prev) == 0 || would_merge(&prev, node))
                        {
                            let diff = prev_match.end - previous_length;
                            let concat_text = utf16::slice(&text, 0, diff);
                            let new_text_content = format!("{previous_text}{concat_text}");
                            if Self::get_mode(&prev) == 0 {
                                prev.select(None, None)?;
                            } else {
                                // A token or segmented entity kept above: the
                                // selection stays where it was in the text
                                move_selection_into_entity(node, &prev, diff)?;
                            }
                            // The entity takes the first `diff` characters of
                            // this node (`INTERNAL_$recordTextMove`, with no
                            // text edit log to record it in).
                            prev.set_text_content(&new_text_content);

                            if diff == utf16::len(&text) {
                                node.remove(false)?;
                            } else {
                                let remaining_text = utf16::slice_from(&text, diff);
                                node.set_text_content(&remaining_text);
                            }

                            return Ok(());
                        }
                    }
                    _ => {
                        self.replace_with_simple_text(&prev)?;

                        return Ok(());
                    }
                }
            } else if prev_match.is_none_or(|m| m.start < previous_length) {
                self.check_next_sibling(node, &text)?;
                return Ok(());
            }
        }

        let mut prev_match_length_to_skip = 0;

        loop {
            let remaining_text = std::mem::take(&mut text);
            let found = (self.get_match)(&remaining_text);
            let next_text =
                found.map_or(String::new(), |m| utf16::slice_from(&remaining_text, m.end));
            text = next_text;

            if text.is_empty()
                && let Some(next_sibling) = current_node.get_next_sibling().filter(NodeRef::is_text)
            {
                // The match ends where this node ends, so the next sibling's
                // text may extend, shorten or invalidate it. Re-run getMatch
                // over the text that is still to be processed plus the
                // sibling's text and bail out unless the same match is found
                // at the same offset. Matches that were already replaced (or
                // skipped) are excluded from `remaining_text`, so the
                // comparison is against `match.start` rather than 0.
                let combined_text = format!("{remaining_text}{}", next_sibling.get_text_content());
                match (self.get_match)(&combined_text) {
                    None => {
                        if self.is_target_node(&next_sibling) {
                            // Unless the transforms keep it there, which they
                            // do for a match that needs the text after it in
                            // its source node.
                            if !self.is_entity_after(&remaining_text, &next_sibling, false) {
                                self.replace_with_simple_text(&next_sibling)?;
                            }
                        } else {
                            next_sibling.mark_dirty();
                        }

                        return Ok(());
                    }
                    Some(next_match) => {
                        if found.is_none_or(|m| next_match.start != m.start) {
                            return Ok(());
                        }
                    }
                }
            }

            let Some(found) = found else {
                return Ok(());
            };

            if found.start == 0
                && let Some(prev) = prev_sibling.as_ref().filter(|n| n.is_text())
                && (prev.is_text_entity()
                    // The transform of a simple text node checks the entity
                    // after it with its own text in front, and turns it back
                    // into text if that is no match. Leave a match there as
                    // text (and any match right after it) rather than make an
                    // entity that check undoes, which would make the two
                    // transforms undo each other forever (#9637).
                    || (prev.is_simple_text()
                        && found.end > 0
                        && (prev_match_length_to_skip > 0
                            || ((self.get_match)(&format!(
                                "{}{}",
                                prev.get_text_content(),
                                utf16::slice(&remaining_text, 0, found.end)
                            ))
                            .is_none()
                                // Unless it is this match with the text after
                                // it too.
                                && !first_match.is_some_and(|first| {
                                    first.start == prev.get_text_content_size()
                                        && first.end == prev.get_text_content_size() + found.end
                                })))))
            {
                prev_match_length_to_skip += found.end;
                continue;
            }

            let (node_to_replace, rest) = if found.start == 0 {
                let mut parts = current_node.split_text(&[found.end as i64])?.into_iter();
                (parts.next(), parts.next())
            } else {
                let mut parts = current_node
                    .split_text(&[
                        (found.start + prev_match_length_to_skip) as i64,
                        (found.end + prev_match_length_to_skip) as i64,
                    ])?
                    .into_iter();
                parts.next();
                (parts.next(), parts.next())
            };

            invariant!(
                node_to_replace.is_some(),
                "{} should not be undefined. You may want to check splitOffsets passed to the splitText.",
                "nodeToReplace"
            );
            let node_to_replace = node_to_replace.expect("checked");

            let replacement_node = (self.create_node)(&node_to_replace);
            let data = node_to_replace.text_data();
            replacement_node.set_format(data.format);
            replacement_node.set_style(&data.style);
            replacement_node.set_detail(data.detail)?;
            node_to_replace.replace(&replacement_node, false)?;

            let Some(rest) = rest else {
                return Ok(());
            };
            current_node = rest;
            prev_match_length_to_skip = 0;
            prev_sibling = Some(replacement_node);
        }
    }

    /// `$reverseNodeTransform(node)`.
    fn reverse_node_transform(&self, node: &NodeRef) -> Result<(), LexicalError> {
        let text = node.get_text_content();
        let Some(found) = (self.get_match)(&text).filter(|m| m.start == 0) else {
            self.replace_with_simple_text(node)?;

            return Ok(());
        };

        if utf16::len(&text) > found.end {
            // This will split out the rest of the text as simple text
            node.split_text(&[found.end as i64])?;

            return Ok(());
        }

        let prev_sibling = node.get_previous_sibling().filter(NodeRef::is_text);

        if let Some(prev) = &prev_sibling
            && prev.is_simple_text()
            // The text node before it checks it on its own while it is a
            // match after its text.
            && !Self::is_next_text_entity(node)
            && !self.is_entity_after(&prev.get_text_content(), node, true)
        {
            // The text after this entity may have changed so that the
            // transforms no longer keep it after the text before it: the
            // transform of the text node before it turns it back into text
            // then. Once per pending editor state, as the transforms of other
            // entities may leave it dirty again.
            let update_state = self.update_state();
            let first_time = self.marked_in_update.with(&update_state, |marked| {
                marked.insert(prev.get_key().to_owned())
            });
            if first_time {
                prev.mark_dirty();
            }
        }

        // Two entities next to each other are turned back into text, unless
        // this entity was already turned back into text for that twice for
        // this pending editor state: entities of two registrations that touch
        // would be made again and turned back forever. They are kept then.
        if let Some(prev) = &prev_sibling
            && prev.is_text_entity()
            && !self.was_turned_into_text(node, "adjacent:")
        {
            self.replace_with_simple_text(prev)?;
            self.replace_with_simple_text(node)?;
        }

        if let Some(next_sibling) = node.get_next_sibling().filter(is_text_entity)
            // This may have already been converted in the previous block
            && (!self.is_target_node(node) || !self.was_turned_into_text(node, "adjacent:"))
        {
            self.replace_with_simple_text(&next_sibling)?;

            if self.is_target_node(node) {
                self.replace_with_simple_text(node)?;
            }
        }
        Ok(())
    }
}

/// `registerLexicalTextEntity(editor, getMatch, targetNode, createNode)`:
/// a transform of `text` nodes that makes an entity of `target_type` with
/// `create_node` of each match `get_match` finds in simple text (in the
/// text's format, style and detail), and one of `target_type` that turns an
/// entity back into text, or splits the text past its match off it, when
/// an edit changes its match. Each transform runs on its own type only, as
/// in JS. Returns what unregisters each, in that order.
pub fn register_lexical_text_entity(
    editor: &Editor,
    get_match: impl Fn(&str) -> Option<EntityMatch> + 'static,
    target_type: &'static str,
    create_node: impl Fn(&NodeRef) -> NodeRef + 'static,
) -> Vec<Unregister> {
    let entity = Rc::new(TextEntity {
        editor: editor.weak(),
        get_match: Box::new(get_match),
        target_type,
        create_node: Box::new(create_node),
        marked_in_update: InUpdate::new(),
        turned_into_text_in_update: InUpdate::new(),
    });

    let text_entity = entity.clone();
    let remove_plain_text_transform =
        editor.register_node_transform("text", move |node| text_entity.text_node_transform(node));
    let remove_reverse_node_transform = editor
        .register_node_transform(target_type, move |node| entity.reverse_node_transform(node));

    vec![remove_plain_text_transform, remove_reverse_node_transform]
}
