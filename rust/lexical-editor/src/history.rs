//! `@lexical/history` (`packages/lexical-history/src/index.ts`): undo and
//! redo. [`register_history`] records the editor's states on an undo stack,
//! merging an update into the entry before it when it continues the same
//! kind of change (typing, deleting) within a delay, and handles
//! [`UNDO_COMMAND`], [`REDO_COMMAND`], [`CLEAR_HISTORY_COMMAND`] and
//! [`CLEAR_EDITOR_COMMAND`], dispatching [`CAN_UNDO_COMMAND`] and
//! [`CAN_REDO_COMMAND`] as the stacks fill and empty.
//!
//! `HistoryExtension` and `SharedHistoryExtension` wait on the extension
//! API; a [`HistoryState`] passed to `register_history` for several editors
//! is shared history all the same. The delay and the depth are plain values
//! where JS also takes signals.

use crate::commands::{
    CAN_REDO_COMMAND, CAN_UNDO_COMMAND, CLEAR_EDITOR_COMMAND, CLEAR_HISTORY_COMMAND, REDO_COMMAND,
    UNDO_COMMAND,
};
use crate::editor::{
    COMMAND_PRIORITY_EDITOR, COMPOSITION_END_TAG, COMPOSITION_START_TAG, CUT_TAG, EditorInner,
    HISTORIC_TAG, HISTORY_MERGE_TAG, HISTORY_PUSH_TAG, LexicalCommand, PASTE_TAG, Unregister,
    UpdateListenerPayload,
};
use crate::json::Value;
use crate::lexical_selection::{PointType, RangeSelection};
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::node_state::export_json_with_state;
use crate::nodes::root::RootNode;
use crate::ordered::OrderedSet;
use crate::{Editor, EditorState, LexicalError, utf16};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

/// `MergeAction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MergeAction {
    /// `HISTORY_MERGE`: the update replaces the current entry.
    HistoryMerge,
    /// `HISTORY_PUSH`: the current entry goes on the undo stack.
    HistoryPush,
    /// `DISCARD_HISTORY_CANDIDATE`: the update isn't recorded.
    DiscardHistoryCandidate,
}

/// `ChangeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChangeType {
    Other,
    ComposingCharacter,
    InsertCharacterAfterSelection,
    DeleteCharacterBeforeSelection,
    DeleteCharacterAfterSelection,
}

/// `HistoryStateEntry`: an editor and one of its states.
#[derive(Clone)]
pub struct HistoryStateEntry {
    pub editor: Editor,
    pub editor_state: EditorState,
}

/// `HistoryState`: the `current` entry, and the undo and redo stacks of
/// previous and future entries. [`create_empty_history_state`] makes an
/// empty one, shared through an `Rc` as JS shares the object; pass the same
/// one to [`register_history`] for several editors to share their history.
#[derive(Clone, Default)]
pub struct HistoryState {
    pub current: Option<HistoryStateEntry>,
    pub redo_stack: Vec<HistoryStateEntry>,
    pub undo_stack: Vec<HistoryStateEntry>,
}

/// The clock [`register_history`] merges by: milliseconds, as `Date.now()`
/// gives them.
pub type DateNow = Rc<dyn Fn() -> f64>;

/// What [`register_history`] calls when the history state changes
/// (`onHistoryStateChange`). It gets the state borrowed, so it must not
/// change it.
pub type HistoryStateChange = Rc<dyn Fn(&HistoryState)>;

/// `Date.now()`: milliseconds since the Unix epoch, the default clock.
fn system_date_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_millis() as f64)
}

/// `getDirtyNodes(editorState, dirtyLeaves, dirtyElements)`: the dirty
/// leaves, then the elements marked dirty themselves, but not the root.
fn get_dirty_nodes(
    editor_state: &EditorState,
    dirty_leaves: &[NodeKey],
    dirty_elements: &[(NodeKey, bool)],
) -> Vec<Rc<dyn LexicalNode>> {
    let node_map = editor_state.0.node_map.borrow();
    let mut nodes: Vec<Rc<dyn LexicalNode>> = dirty_leaves
        .iter()
        .filter_map(|key| node_map.get(key).cloned())
        .collect();
    for (key, intentionally_marked_as_dirty) in dirty_elements {
        if !intentionally_marked_as_dirty {
            continue;
        }
        if let Some(dirty_element) = node_map.get(key)
            && !dirty_element.is::<RootNode>()
        {
            nodes.push(dirty_element.clone());
        }
    }
    nodes
}

fn range_selection(editor_state: &EditorState) -> Option<RangeSelection> {
    editor_state.selection().and_then(|s| s.range())
}

/// `getChangeType(prevEditorState, nextEditorState, dirtyLeavesSet,
/// dirtyElementsSet, isComposing)`.
fn get_change_type(
    prev_editor_state: &EditorState,
    next_editor_state: &EditorState,
    dirty_leaves: &[NodeKey],
    dirty_elements: &[(NodeKey, bool)],
    is_composing: bool,
) -> ChangeType {
    // An update listener always has a previous state, which JS checks for
    // `null` here.
    if dirty_leaves.is_empty() && dirty_elements.is_empty() && !is_composing {
        return ChangeType::Other;
    }
    if is_composing {
        return ChangeType::ComposingCharacter;
    }
    let (Some(next_selection), Some(prev_selection)) = (
        range_selection(next_editor_state),
        range_selection(prev_editor_state),
    ) else {
        return ChangeType::Other;
    };
    if !prev_selection.is_collapsed() || !next_selection.is_collapsed() {
        return ChangeType::Other;
    }
    let dirty_nodes = get_dirty_nodes(next_editor_state, dirty_leaves, dirty_elements);
    if dirty_nodes.is_empty() {
        return ChangeType::Other;
    }
    let next_node_map = next_editor_state.0.node_map.borrow();
    let prev_node_map = prev_editor_state.0.node_map.borrow();

    // Catching the case when inserting new text node into an element (e.g.
    // first char in paragraph/list), or after existing node.
    if dirty_nodes.len() > 1 {
        let next_anchor_node = next_node_map.get(&next_selection.anchor.key);
        // Read from the next state, as JS does.
        let prev_anchor_node = next_node_map.get(&prev_selection.anchor.key);
        if let (Some(next_anchor_node), Some(_)) = (next_anchor_node, prev_anchor_node)
            && !prev_node_map.contains_key(next_anchor_node.get_key())
            && let Some(text) = next_anchor_node.text()
            && utf16::len(&text.text) == 1
            && next_selection.anchor.offset == 1
        {
            return ChangeType::InsertCharacterAfterSelection;
        }
        return ChangeType::Other;
    }

    let next_dirty_node = &dirty_nodes[0];
    let prev_dirty_node = prev_node_map.get(next_dirty_node.get_key());
    let (Some(prev_text), Some(next_text)) = (
        prev_dirty_node.and_then(|n| n.text()),
        next_dirty_node.text(),
    ) else {
        return ChangeType::Other;
    };
    if prev_text.mode != next_text.mode || prev_text.text == next_text.text {
        return ChangeType::Other;
    }
    let next_anchor = &next_selection.anchor;
    let prev_anchor = &prev_selection.anchor;
    if next_anchor.key != prev_anchor.key || next_anchor.point_type != PointType::Text {
        return ChangeType::Other;
    }
    let next_anchor_offset = next_anchor.offset as i64;
    let prev_anchor_offset = prev_anchor.offset as i64;
    let text_diff = utf16::len(&next_text.text) as i64 - utf16::len(&prev_text.text) as i64;
    if text_diff == 1 && prev_anchor_offset == next_anchor_offset - 1 {
        return ChangeType::InsertCharacterAfterSelection;
    }
    if text_diff == -1 && prev_anchor_offset == next_anchor_offset + 1 {
        return ChangeType::DeleteCharacterBeforeSelection;
    }
    if text_diff == -1 && prev_anchor_offset == next_anchor_offset {
        return ChangeType::DeleteCharacterAfterSelection;
    }
    ChangeType::Other
}

fn get_node(editor_state: &EditorState, key: &str) -> Option<Rc<dyn LexicalNode>> {
    editor_state.0.node_map.borrow().get(key).cloned()
}

/// `JSON.stringify(node.exportJSON())`, read in `editor_state`.
fn export_json_text(editor_state: &EditorState, node: &Rc<dyn LexicalNode>) -> String {
    editor_state.read(|| Value::Object(export_json_with_state(&**node, false)).to_string())
}

/// `isTextNodeUnchanged(key, prevEditorState, nextEditorState)`.
fn is_text_node_unchanged(
    key: &str,
    prev_editor_state: &EditorState,
    next_editor_state: &EditorState,
) -> bool {
    let prev_node = get_node(prev_editor_state, key);
    let next_node = get_node(next_editor_state, key);
    let is_deleting_line = match (
        range_selection(prev_editor_state),
        range_selection(next_editor_state),
    ) {
        (Some(prev), Some(next)) => {
            prev.anchor.point_type == PointType::Element
                && prev.focus.point_type == PointType::Element
                && next.anchor.point_type == PointType::Text
                && next.focus.point_type == PointType::Text
        }
        _ => false,
    };
    if !is_deleting_line
        && let (Some(prev_node), Some(next_node)) = (prev_node, next_node)
        && prev_node.text().is_some()
        && next_node.text().is_some()
        && prev_node.base().parent == next_node.base().parent
    {
        // This has the assumption that object key order won't change if the
        // content did not change, which should normally be safe given the
        // manner in which nodes and exportJSON are typically implemented.
        return export_json_text(prev_editor_state, &prev_node)
            == export_json_text(next_editor_state, &next_node);
    }
    false
}

/// `$serializeNode(node)`: the node's JSON and, for an element, its
/// children's keys.
fn serialize_node(node: &Rc<dyn LexicalNode>) -> String {
    let children_keys = if node.element().is_some() {
        Value::Array(
            NodeRef::new(node.get_key())
                .get_children()
                .iter()
                .map(|child| child.get_key().into())
                .collect(),
        )
    } else {
        Value::Null
    };
    Value::Array(vec![
        Value::Object(export_json_with_state(&**node, false)),
        children_keys,
    ])
    .to_string()
}

/// `areNodesUnchanged(keys, prevEditorState, nextEditorState)`.
fn are_nodes_unchanged(
    keys: &OrderedSet,
    prev_editor_state: &EditorState,
    next_editor_state: &EditorState,
) -> bool {
    for key in keys.iter() {
        match (
            get_node(prev_editor_state, key),
            get_node(next_editor_state, key),
        ) {
            (None, None) => continue,
            (Some(prev_node), Some(next_node)) if Rc::ptr_eq(&prev_node, &next_node) => continue,
            (Some(prev_node), Some(next_node)) => {
                if prev_editor_state.read(|| serialize_node(&prev_node))
                    != next_editor_state.read(|| serialize_node(&next_node))
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// The state of `createMergeActionGetter(editor, delay, dateNow)`'s
/// closure; [`get`](Self::get) is the function it returns.
struct MergeActionGetter {
    delay: f64,
    date_now: DateNow,
    prev_change_time: f64,
    prev_change_type: ChangeType,
    composition_start_time: f64,
    composition_start_change_type: ChangeType,
    composition_start_state: Option<EditorState>,
    /// Include every update in the composition, not only its final
    /// deletion.
    composition_dirty_keys: OrderedSet,
}

impl MergeActionGetter {
    /// `createMergeActionGetter(editor, delay, dateNow)`.
    fn new(delay: f64, date_now: DateNow) -> Self {
        let prev_change_time = date_now();
        Self {
            delay,
            date_now,
            prev_change_time,
            prev_change_type: ChangeType::Other,
            composition_start_time: prev_change_time,
            composition_start_change_type: ChangeType::Other,
            composition_start_state: None,
            composition_dirty_keys: OrderedSet::new(),
        }
    }

    /// The function `createMergeActionGetter` returns, with
    /// `prevEditorState`, `nextEditorState`, `dirtyLeaves`, `dirtyElements`
    /// and `tags` from the update listener's payload.
    fn get(
        &mut self,
        editor: &Editor,
        current_history_entry: Option<&HistoryStateEntry>,
        payload: &UpdateListenerPayload,
    ) -> MergeAction {
        let next_editor_state = &payload.editor_state;
        let dirty_leaves = &payload.dirty_leaves;
        let dirty_elements = &payload.dirty_elements;
        let tags = &payload.tags;
        let has = |tag: &str| tags.iter().any(|t| t == tag);
        let change_time = (self.date_now)();

        if has(COMPOSITION_START_TAG) {
            self.composition_start_time = self.prev_change_time;
            self.composition_start_change_type = self.prev_change_type;
            self.composition_start_state = Some(payload.prev_editor_state.clone());
            self.composition_dirty_keys = OrderedSet::new();
        }
        if self.composition_start_state.is_some() {
            for key in dirty_leaves {
                self.composition_dirty_keys.add(key);
            }
            for (key, _) in dirty_elements {
                self.composition_dirty_keys.add(key);
            }
        }

        // If applying changes from history stack there's no need to run
        // history logic again, as history entries already calculated.
        if has(HISTORIC_TAG) {
            self.prev_change_type = ChangeType::Other;
            self.prev_change_time = change_time;
            self.composition_start_state = None;
            self.composition_dirty_keys = OrderedSet::new();
            return MergeAction::DiscardHistoryCandidate;
        }

        let is_composition_end = has(COMPOSITION_END_TAG);
        let mut prev_editor_state = payload.prev_editor_state.clone();
        if is_composition_end && let Some(start) = &self.composition_start_state {
            self.prev_change_time = self.composition_start_time;
            self.prev_change_type = self.composition_start_change_type;
            prev_editor_state = start.clone();
        }

        // A paste or cut is never a continuation of the surrounding typing,
        // so it is OTHER: it doesn't merge into the keystrokes before it, and
        // as `prev_change_type` it stops the keystrokes after it from
        // merging into it, which gives it its own undo entry (#8609).
        let change_type = if has(PASTE_TAG) || has(CUT_TAG) {
            ChangeType::Other
        } else {
            // JS passes `editor.isComposing()`. Composition isn't ported
            // (rust/PARITY.md), so it is false, as in a headless JS editor.
            get_change_type(
                &prev_editor_state,
                next_editor_state,
                dirty_leaves,
                dirty_elements,
                false,
            )
        };

        let mut is_unchanged_composition = false;
        let merge_action = 'merge_action: {
            let is_same_editor =
                current_history_entry.is_none_or(|entry| entry.editor.ptr_eq(editor));
            let should_push_history = has(HISTORY_PUSH_TAG);
            let should_merge_history =
                !should_push_history && is_same_editor && has(HISTORY_MERGE_TAG);

            if is_composition_end
                && let Some(start) = &self.composition_start_state
                && is_same_editor
                && !should_push_history
                && are_nodes_unchanged(&self.composition_dirty_keys, start, next_editor_state)
            {
                is_unchanged_composition = true;
                break 'merge_action MergeAction::HistoryMerge;
            }

            if should_merge_history {
                break 'merge_action MergeAction::HistoryMerge;
            }

            if change_type == ChangeType::ComposingCharacter {
                break 'merge_action MergeAction::DiscardHistoryCandidate;
            }

            // JS pushes when there is no previous state, which an update
            // listener always has.

            let has_dirty_nodes = !dirty_leaves.is_empty() || !dirty_elements.is_empty();
            if !has_dirty_nodes {
                if next_editor_state.selection().is_some() {
                    break 'merge_action MergeAction::HistoryMerge;
                }
                break 'merge_action MergeAction::DiscardHistoryCandidate;
            }

            if !should_push_history
                && change_type != ChangeType::Other
                && change_type == self.prev_change_type
                && change_time < self.prev_change_time + self.delay
                && is_same_editor
            {
                break 'merge_action MergeAction::HistoryMerge;
            }

            // A single node might have been marked as dirty, but not have
            // changed due to some node transform reverting the change. An
            // explicit HISTORY_PUSH_TAG always asks for its own entry, so it
            // must not be merged away here even when the only dirty leaf is
            // an unchanged TextNode (#9217).
            if !should_push_history
                && dirty_leaves.len() == 1
                && is_text_node_unchanged(&dirty_leaves[0], &prev_editor_state, next_editor_state)
            {
                break 'merge_action MergeAction::HistoryMerge;
            }

            MergeAction::HistoryPush
        };

        if is_composition_end {
            self.composition_start_state = None;
            self.composition_dirty_keys = OrderedSet::new();
        }
        if !is_unchanged_composition {
            self.prev_change_time = change_time;
            self.prev_change_type = change_type;
        }
        merge_action
    }
}

/// `getInverseEntry(historyStateEntry, current)`: the entry that reverses
/// `history_state_entry`, for the opposite stack.
///
/// With a single editor this is always `current`, since `current` tracks
/// the live state of that editor. With a shared [`HistoryState`] the stacks
/// interleave entries from several editors, so `current` may belong to an
/// editor that is not about to change; then it is the live state of the
/// entry's own editor.
fn get_inverse_entry(
    history_state_entry: &HistoryStateEntry,
    current: Option<&HistoryStateEntry>,
) -> Option<HistoryStateEntry> {
    if let Some(current) = current
        && current.editor.ptr_eq(&history_state_entry.editor)
    {
        return Some(current.clone());
    }
    let editor = &history_state_entry.editor;
    let editor_state = editor.get_editor_state();
    // An empty EditorState can not be restored (setEditorState fails), so
    // there is nothing to reverse to.
    (!editor_state.is_empty()).then(|| HistoryStateEntry {
        editor: editor.clone(),
        editor_state,
    })
}

/// `getUndoEntry(editor, prevEditorState, current)`: the entry an update
/// from `editor` pushes onto the undo stack: `current`, or with a shared
/// [`HistoryState`] whose `current` belongs to another editor, this
/// editor's own state before the update.
fn get_undo_entry(
    editor: &Editor,
    prev_editor_state: &EditorState,
    current: Option<&HistoryStateEntry>,
) -> Option<HistoryStateEntry> {
    let current = current?;
    if current.editor.ptr_eq(editor) {
        return Some(current.clone());
    }
    // An empty EditorState can not be restored (setEditorState fails).
    // Skipping it mirrors the way the first update of an editor is not
    // undoable.
    (!prev_editor_state.is_empty()).then(|| HistoryStateEntry {
        editor: editor.clone(),
        editor_state: prev_editor_state.clone(),
    })
}

fn notify(history_state: &RefCell<HistoryState>, on_change: Option<&HistoryStateChange>) {
    if let Some(on_change) = on_change {
        on_change(&history_state.borrow());
    }
}

/// `redo(editor, historyState, onChange)`.
fn redo(
    editor: &Editor,
    history_state: &RefCell<HistoryState>,
    on_change: Option<&HistoryStateChange>,
) -> Result<(), LexicalError> {
    let Some(history_state_entry) = history_state.borrow_mut().redo_stack.pop() else {
        return Ok(());
    };
    let current = history_state.borrow().current.clone();
    if let Some(inverse_entry) = get_inverse_entry(&history_state_entry, current.as_ref()) {
        history_state.borrow_mut().undo_stack.push(inverse_entry);
        editor.dispatch_command(&CAN_UNDO_COMMAND, true)?;
    }
    if history_state.borrow().redo_stack.is_empty() {
        editor.dispatch_command(&CAN_REDO_COMMAND, false)?;
    }
    history_state.borrow_mut().current = Some(history_state_entry.clone());
    notify(history_state, on_change);
    history_state_entry
        .editor
        .set_editor_state(&history_state_entry.editor_state, Some(HISTORIC_TAG))
}

/// `undo(editor, historyState, onChange)`.
fn undo(
    editor: &Editor,
    history_state: &RefCell<HistoryState>,
    on_change: Option<&HistoryStateChange>,
) -> Result<(), LexicalError> {
    let Some(history_state_entry) = history_state.borrow_mut().undo_stack.pop() else {
        return Ok(());
    };
    let current = history_state.borrow().current.clone();
    if let Some(inverse_entry) = get_inverse_entry(&history_state_entry, current.as_ref()) {
        history_state.borrow_mut().redo_stack.push(inverse_entry);
        editor.dispatch_command(&CAN_REDO_COMMAND, true)?;
    }
    if history_state.borrow().undo_stack.is_empty() {
        editor.dispatch_command(&CAN_UNDO_COMMAND, false)?;
    }
    history_state.borrow_mut().current = Some(history_state_entry.clone());
    notify(history_state, on_change);
    history_state_entry
        .editor
        .set_editor_state(&history_state_entry.editor_state, Some(HISTORIC_TAG))
}

/// `clearHistory(historyState, onChange)`.
fn clear_history(history_state: &RefCell<HistoryState>, on_change: Option<&HistoryStateChange>) {
    {
        let mut state = history_state.borrow_mut();
        state.undo_stack = Vec::new();
        state.redo_stack = Vec::new();
        state.current = None;
    }
    notify(history_state, on_change);
}

/// Dispatches `CAN_UNDO_COMMAND` or `CAN_REDO_COMMAND` from the update
/// listener, which has no caller to give an error to: a handler's failed
/// update is discarded, as in JS.
fn dispatch_from_listener(editor: &Editor, command: &LexicalCommand<bool>, payload: bool) {
    let _ = editor.dispatch_command(command, payload);
}

/// `registerHistory(editor, historyState, delay, dateNow,
/// onHistoryStateChange, maxDepth)`: records the editor's updates in
/// `history_state` and handles the undo, redo and clear commands. Returns
/// the function that unregisters it all.
///
/// - `delay`: how long, in milliseconds, an update of the same kind as the
///   one before it (typing, deleting backward, deleting forward) still
///   merges into its entry instead of pushing a new one.
/// - `date_now`: the clock, in milliseconds; `None` is the system clock
///   (`Date.now`).
/// - `on_history_state_change`: called once now, and again whenever
///   `history_state` changes (not when an update is discarded).
/// - `max_depth`: the most entries the undo stack keeps; the oldest go
///   first. `None` keeps them all.
///
/// The editor's listeners hold `history_state`, and its entries hold their
/// editors, so the editor stays alive until this is unregistered.
pub fn register_history(
    editor: &Editor,
    history_state: &Rc<RefCell<HistoryState>>,
    delay: f64,
    date_now: Option<DateNow>,
    on_history_state_change: Option<HistoryStateChange>,
    max_depth: Option<usize>,
) -> Unregister {
    let date_now = date_now.unwrap_or_else(|| Rc::new(system_date_now));
    let get_merge_action = RefCell::new(MergeActionGetter::new(delay, date_now));
    let weak: Weak<EditorInner> = Rc::downgrade(&editor.0);

    let apply_change = {
        let history_state = history_state.clone();
        let on_change = on_history_state_change.clone();
        move |payload: &UpdateListenerPayload| {
            let Some(editor) = weak.upgrade().map(Editor) else {
                return;
            };
            let current = history_state.borrow().current.clone();
            if let Some(current) = &current
                && payload.editor_state.ptr_eq(&current.editor_state)
            {
                return;
            }
            let merge_action =
                get_merge_action
                    .borrow_mut()
                    .get(&editor, current.as_ref(), payload);
            match merge_action {
                MergeAction::HistoryPush => {
                    if !history_state.borrow().redo_stack.is_empty() {
                        history_state.borrow_mut().redo_stack = Vec::new();
                        dispatch_from_listener(&editor, &CAN_REDO_COMMAND, false);
                    }
                    let undo_entry =
                        get_undo_entry(&editor, &payload.prev_editor_state, current.as_ref());
                    if let Some(undo_entry) = undo_entry {
                        {
                            let mut state = history_state.borrow_mut();
                            state.undo_stack.push(undo_entry);
                            if let Some(cap) = max_depth
                                && state.undo_stack.len() > cap
                            {
                                // FIFO-evict the oldest entries so the stack
                                // stays at `cap`.
                                let excess = state.undo_stack.len() - cap;
                                state.undo_stack.drain(..excess);
                            }
                        }
                        dispatch_from_listener(&editor, &CAN_UNDO_COMMAND, true);
                    }
                }
                MergeAction::DiscardHistoryCandidate => return,
                MergeAction::HistoryMerge => {}
            }
            history_state.borrow_mut().current = Some(HistoryStateEntry {
                editor,
                editor_state: payload.editor_state.clone(),
            });
            notify(&history_state, on_change.as_ref());
        }
    };

    // Lets the caller read pre-populated stacks right away, without waiting
    // for the first edit.
    notify(history_state, on_history_state_change.as_ref());

    let unregisters: Vec<Unregister> = vec![
        {
            let history_state = history_state.clone();
            let on_change = on_history_state_change.clone();
            editor.register_command(
                &UNDO_COMMAND,
                move |_, editor| {
                    undo(editor, &history_state, on_change.as_ref())?;
                    Ok(true)
                },
                COMMAND_PRIORITY_EDITOR,
            )
        },
        {
            let history_state = history_state.clone();
            let on_change = on_history_state_change.clone();
            editor.register_command(
                &REDO_COMMAND,
                move |_, editor| {
                    redo(editor, &history_state, on_change.as_ref())?;
                    Ok(true)
                },
                COMMAND_PRIORITY_EDITOR,
            )
        },
        {
            let history_state = history_state.clone();
            let on_change = on_history_state_change.clone();
            editor.register_command(
                &CLEAR_EDITOR_COMMAND,
                move |_, _| {
                    clear_history(&history_state, on_change.as_ref());
                    Ok(false)
                },
                COMMAND_PRIORITY_EDITOR,
            )
        },
        {
            let history_state = history_state.clone();
            let on_change = on_history_state_change;
            editor.register_command(
                &CLEAR_HISTORY_COMMAND,
                move |_, editor| {
                    clear_history(&history_state, on_change.as_ref());
                    editor.dispatch_command(&CAN_REDO_COMMAND, false)?;
                    editor.dispatch_command(&CAN_UNDO_COMMAND, false)?;
                    Ok(true)
                },
                COMMAND_PRIORITY_EDITOR,
            )
        },
        editor.register_update_listener(apply_change),
    ];
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}

/// `createEmptyHistoryState()`.
pub fn create_empty_history_state() -> Rc<RefCell<HistoryState>> {
    Rc::new(RefCell::new(HistoryState::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CreateEditorArgs, UpdateOptions, create_editor, create_paragraph_node, create_text_node,
        get_root,
    };
    use std::cell::Cell;

    fn discrete() -> UpdateOptions {
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        }
    }

    /// `$setParagraphText(text)` in an update of its own.
    fn set_paragraph_text(editor: &Editor, text: &'static str) {
        editor
            .update(
                move || {
                    let root = get_root();
                    root.clear()?;
                    let paragraph = create_paragraph_node();
                    paragraph.append(&[create_text_node(text)])?;
                    root.append(&[paragraph])
                },
                discrete(),
            )
            .unwrap();
    }

    /// The editor's text, after committing a dispatch's update (which JS's
    /// tests `await`).
    fn text(editor: &Editor) -> String {
        editor.read(|| get_root().get_text_content()).unwrap()
    }

    /// Upstream's "undo and redo apply to the editor that changed" (#8623):
    /// two editors sharing one history state.
    #[test]
    fn shared_history_undoes_the_editor_that_changed() {
        let parent = create_editor(CreateEditorArgs::default());
        let nested = create_editor(CreateEditorArgs::default());
        // Content before history, so the edits below are changes rather
        // than initialization.
        for editor in [&parent, &nested] {
            set_paragraph_text(editor, "initial");
        }
        let shared = create_empty_history_state();
        let unregister_parent = register_history(&parent, &shared, 0.0, None, None, None);
        let unregister_nested = register_history(&nested, &shared, 0.0, None, None, None);

        set_paragraph_text(&parent, "parent 1");
        set_paragraph_text(&parent, "parent 2");
        set_paragraph_text(&nested, "nested 1");
        {
            let state = shared.borrow();
            assert_eq!(state.undo_stack.len(), 2);
            assert!(state.undo_stack[0].editor.ptr_eq(&parent));
            assert!(state.undo_stack[1].editor.ptr_eq(&nested));
        }

        // The most recent change was in the nested editor.
        nested.dispatch_command(&UNDO_COMMAND, ()).unwrap();
        assert_eq!(text(&nested), "initial");
        assert_eq!(text(&parent), "parent 2");

        // The next entry belongs to the parent editor.
        nested.dispatch_command(&UNDO_COMMAND, ()).unwrap();
        assert_eq!(text(&nested), "initial");
        assert_eq!(text(&parent), "parent 1");
        assert!(shared.borrow().undo_stack.is_empty());

        nested.dispatch_command(&REDO_COMMAND, ()).unwrap();
        assert_eq!(text(&nested), "initial");
        assert_eq!(text(&parent), "parent 2");

        nested.dispatch_command(&REDO_COMMAND, ()).unwrap();
        assert_eq!(text(&nested), "nested 1");
        assert_eq!(text(&parent), "parent 2");
        assert!(shared.borrow().redo_stack.is_empty());

        unregister_parent();
        unregister_nested();
    }

    /// Upstream's `maxDepth` tests: `n` separate events leave `n - 1`
    /// entries, and a cap keeps the newest.
    #[test]
    fn max_depth_evicts_the_oldest_entries() {
        for (max_depth, expected) in [(None, 24), (Some(5), 5)] {
            let editor = create_editor(CreateEditorArgs::default());
            let history_state = create_empty_history_state();
            let unregister = register_history(&editor, &history_state, 0.0, None, None, max_depth);
            for i in 0..25 {
                editor
                    .update(
                        move || {
                            let paragraph = create_paragraph_node();
                            paragraph.append(&[create_text_node(&format!("p{i}"))])?;
                            get_root().append(&[paragraph])
                        },
                        discrete(),
                    )
                    .unwrap();
            }
            let state = history_state.borrow();
            assert_eq!(state.undo_stack.len(), expected, "{max_depth:?}");
            // The newest entry is the state before the last paragraph.
            let last = state.undo_stack.last().unwrap();
            assert_eq!(
                last.editor_state.read(|| get_root().get_children_size()),
                24
            );
            drop(state);
            unregister();
        }
    }

    /// `onHistoryStateChange` runs on registration and on every change of
    /// the stacks, but not for a discarded update; the injected clock
    /// decides merging; unregistering stops it all.
    #[test]
    fn on_change_clock_and_unregister() {
        let editor = create_editor(CreateEditorArgs::default());
        let history_state = create_empty_history_state();
        let changes = Rc::new(Cell::new(0));
        let now = Rc::new(Cell::new(0.0));
        let unregister = {
            let (changes, now) = (changes.clone(), now.clone());
            register_history(
                &editor,
                &history_state,
                300.0,
                Some(Rc::new(move || now.get())),
                Some(Rc::new(move |_: &HistoryState| {
                    changes.set(changes.get() + 1)
                })),
                None,
            )
        };
        assert_eq!(changes.get(), 1);
        set_paragraph_text(&editor, "a");
        assert_eq!(changes.get(), 2);
        // Nothing dirty and no selection: discarded.
        editor.update(|| Ok(()), discrete()).unwrap();
        assert_eq!(changes.get(), 2);
        // A different change each time: these push however close they are.
        now.set(10.0);
        set_paragraph_text(&editor, "b");
        assert_eq!(history_state.borrow().undo_stack.len(), 1);
        editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
        assert_eq!(text(&editor), "a");
        editor.dispatch_command(&CLEAR_HISTORY_COMMAND, ()).unwrap();
        assert!(history_state.borrow().current.is_none());
        let before = changes.get();
        unregister();
        set_paragraph_text(&editor, "c");
        assert_eq!(changes.get(), before);
        assert!(!editor.dispatch_command(&UNDO_COMMAND, ()).unwrap());
    }
}
