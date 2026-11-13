//! `LexicalUpdates.ts`: the active editor and editor state that `$`
//! functions read, the update cycle (`editor.update`, transforms, the
//! commit and its listeners), command dispatch, and `parseEditorState`.
//!
//! A headless editor has no DOM, so the reconciler, mutation listeners,
//! selection-change notifications and the DOM selection never run here, as
//! in JS's headless editor. Where JS commits a non-discrete update in a
//! microtask, Rust commits it at the next [`Editor::read`] or discrete
//! update, which JS also does when one comes first.

use crate::editor::{
    DirtyType, LexicalCommand, NodeClass, Transform, UpdateListenerPayload, UpdateOptions,
};
use crate::editor_state::{clone_editor_state, create_empty_editor_state};
use crate::error::invariant;
use crate::gc::garbage_collect_detached_nodes;
use crate::json::Value;
use crate::node::NodeRef;
use crate::normalization::normalize_text_node;
use crate::ordered::{OrderedMap, OrderedSet};
use crate::selection::has_dirty_selection;
use crate::utils::{get_node_by_key, get_root};
use crate::{Editor, EditorState, LexicalError, constants::NODE_STATE_KEY};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

/// JS's module-scope `activeEditor`, `activeEditorState` and
/// `isReadOnlyMode`, per thread.
#[derive(Clone, Default)]
struct Active {
    editor: Option<Editor>,
    editor_state: Option<EditorState>,
    read_only: bool,
}

thread_local! {
    static ACTIVE: RefCell<Active> = RefCell::new(Active::default());
    static INFINITE_TRANSFORM_COUNT: Cell<u32> = const { Cell::new(0) };
}

/// Runs `f` with `active` in place, restoring the previous one afterwards,
/// even when `f` panics.
fn with_active<R>(active: Active, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Active>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take().expect("previous");
            ACTIVE.with(|a| *a.borrow_mut() = previous);
        }
    }
    let previous = ACTIVE.with(|a| std::mem::replace(&mut *a.borrow_mut(), active));
    let _restore = Restore(Some(previous));
    f()
}

/// `getActiveEditorState()`.
pub(crate) fn get_active_editor_state() -> EditorState {
    ACTIVE
        .with(|a| a.borrow().editor_state.clone())
        .unwrap_or_else(|| {
            panic!(
                "Unable to find an active editor state. State helpers or node methods can only be used synchronously during the callback of editor.update(), editor.read(), or editorState.read()."
            )
        })
}

/// `internalGetActiveEditor()`: the editor whose update or read is running,
/// if any. Reading an editor state on its own ([`EditorState::read`],
/// parsing) has none.
pub(crate) fn get_active_editor() -> Option<Editor> {
    ACTIVE.with(|a| a.borrow().editor.clone())
}

/// `isCurrentlyReadOnlyMode()`.
pub(crate) fn is_currently_read_only_mode() -> bool {
    ACTIVE.with(|a| a.borrow().read_only)
}

/// `errorOnReadOnly()`.
pub(crate) fn error_on_read_only() {
    if is_currently_read_only_mode() {
        panic!("Cannot use method in read-only mode.");
    }
}

/// `errorOnInfiniteTransforms()`.
pub(crate) fn error_on_infinite_transforms() {
    if INFINITE_TRANSFORM_COUNT.with(Cell::get) > 99 {
        panic!(
            "One or more transforms are endlessly triggering additional transforms. May have encountered infinite recursion caused by transforms that have their preconditions too lose and/or conflict with each other."
        );
    }
}

fn bump_infinite_transform_count() {
    INFINITE_TRANSFORM_COUNT.with(|c| c.set(c.get() + 1));
}

/// `readEditorState(editor, editorState, callbackFn)`.
pub(crate) fn read_editor_state<R>(
    editor: Option<&Editor>,
    editor_state: &EditorState,
    f: impl FnOnce() -> R,
) -> R {
    with_active(
        Active {
            editor: editor.cloned(),
            editor_state: Some(editor_state.clone()),
            read_only: true,
        },
        f,
    )
}

/// `$applyTransforms(editor, node, transformsCache)`.
fn apply_transforms(
    editor: &Editor,
    node: &NodeRef,
    cache: &mut HashMap<&'static str, Vec<Transform>>,
) -> Result<(), LexicalError> {
    let node_type = node.get_type();
    let transforms = cache
        .entry(node_type)
        .or_insert_with(|| editor.transforms_of(node_type))
        .clone();
    for transform in &transforms {
        transform(node)?;
        if !node.is_attached() {
            break;
        }
    }
    Ok(())
}

/// Whether a dirty leaf is normalized: attached, simple, mergeable text.
fn should_normalize(node: &NodeRef) -> bool {
    node.is_text() && node.is_attached() && node.is_simple_text() && !node.is_unmergeable()
}

/// `$normalizeAllDirtyTextNodes(editorState, editor)`. Iterates the dirty
/// leaves as JS's `for...of` does, seeing those added meanwhile.
fn normalize_all_dirty_text_nodes(editor: &Editor) -> Result<(), LexicalError> {
    let mut i = 0;
    loop {
        let key = editor
            .0
            .dirty
            .borrow()
            .leaves
            .key_at(&mut i)
            .map(str::to_owned);
        let Some(key) = key else { break };
        if let Some(node) = get_node_by_key(&key)
            && should_normalize(&node)
        {
            normalize_text_node(&node)?;
        }
        i += 1;
    }
    Ok(())
}

/// `$applyAllTransforms(editorState, editor)`: transforms dirty leaves
/// first, repeating while they dirty more leaves, then the elements marked
/// dirty themselves, the root last, and again from the leaves while
/// anything new is dirty.
///
/// JS swaps the editor's dirty sets for fresh ones to see what each round
/// dirties, and accumulates the full record in the sets it started with;
/// in the first round the full record is the editor's own set, which is
/// why `full_*` is `None` until the editor's set is first swapped out.
fn apply_all_transforms(editor: &Editor) -> Result<(), LexicalError> {
    let mut cache = HashMap::new();
    let mut full_leaves: Option<OrderedSet> = None;
    let mut full_elements: Option<OrderedMap<bool>> = None;

    loop {
        let (leaves_len, elements_len) = {
            let dirty = editor.0.dirty.borrow();
            (dirty.leaves.len(), dirty.elements.len())
        };
        if leaves_len == 0 && elements_len == 0 {
            break;
        }
        if leaves_len > 0 {
            let untransformed = std::mem::take(&mut editor.0.dirty.borrow_mut().leaves);
            let keys: Vec<String> = untransformed.iter().map(str::to_owned).collect();
            full_leaves.get_or_insert(untransformed);
            for key in &keys {
                if let Some(node) = get_node_by_key(key) {
                    if should_normalize(&node) {
                        normalize_text_node(&node)?;
                    }
                    if node.is_attached() {
                        apply_transforms(editor, &node, &mut cache)?;
                    }
                }
                full_leaves.as_mut().expect("set above").add(key);
            }
            // Leaf transforms come before element transforms.
            if !editor.0.dirty.borrow().leaves.is_empty() {
                bump_infinite_transform_count();
                continue;
            }
        }

        let (previous_leaves, mut untransformed) = {
            let mut dirty = editor.0.dirty.borrow_mut();
            (
                std::mem::take(&mut dirty.leaves),
                std::mem::take(&mut dirty.elements),
            )
        };
        full_leaves.get_or_insert(previous_leaves);
        // The root is always considered intentionally dirty if any attached
        // node is dirty, and moving it last applies its transforms last.
        if untransformed.delete("root").is_some() {
            untransformed.set("root", true);
        }
        full_elements.get_or_insert_with(|| untransformed.clone());
        let entries: Vec<(String, bool)> = untransformed
            .iter()
            .map(|(k, v)| (k.to_owned(), *v))
            .collect();
        for (key, intentionally_marked_as_dirty) in entries {
            full_elements
                .as_mut()
                .expect("set above")
                .set(&key, intentionally_marked_as_dirty);
            if !intentionally_marked_as_dirty {
                continue;
            }
            if let Some(node) = get_node_by_key(&key)
                && node.is_attached()
            {
                apply_transforms(editor, &node, &mut cache)?;
            }
        }
        bump_infinite_transform_count();
    }

    let mut dirty = editor.0.dirty.borrow_mut();
    if let Some(leaves) = full_leaves {
        dirty.leaves = leaves;
    }
    if let Some(elements) = full_elements {
        dirty.elements = elements;
    }
    Ok(())
}

/// `$parseSerializedNodeImpl`.
fn parse_serialized_node_impl(
    serialized_node: &Value,
    registered_nodes: &HashMap<&'static str, NodeClass>,
) -> Result<NodeRef, LexicalError> {
    let Some(object) = serialized_node.as_object() else {
        // `serializedNode.type` of a primitive is `undefined`; of `null` it
        // throws a TypeError.
        invariant!(
            !serialized_node.is_null(),
            "Cannot read properties of null (reading 'type')"
        );
        return Err(LexicalError::new(
            "parseEditorState: type \"undefined\" + not found",
        ));
    };
    let node_type = object.get("type");
    let registered = node_type
        .and_then(Value::as_str)
        .and_then(|t| registered_nodes.get(t));
    let Some(registered) = registered else {
        let shown = match node_type {
            None | Some(Value::Undefined) => "undefined".to_owned(),
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
        };
        return Err(LexicalError::new(format!(
            "parseEditorState: type \"{shown}\" + not found"
        )));
    };
    // Not ported yet; refused rather than dropped (rust/PARITY.md).
    invariant!(
        !object.contains_key(NODE_STATE_KEY),
        "parseEditorState: NodeState (\"$\") is not ported to Rust yet"
    );
    invariant!(
        !object.contains_key("$slots"),
        "parseEditorState: slots (\"$slots\") are not ported to Rust yet"
    );

    let node = (registered.import_json)(object)?;
    if let Some(Value::Array(children)) = object.get("children")
        && node.is_element()
    {
        for child in children {
            let child = parse_serialized_node_impl(child, registered_nodes)?;
            node.append(&[child])?;
        }
    }
    Ok(node)
}

/// `$parseSerializedNode(serializedNode)`: a node, and its children, from
/// its JSON, created in the active update with the active editor's node
/// classes.
pub fn parse_serialized_node(serialized_node: &Value) -> Result<NodeRef, LexicalError> {
    let editor =
        get_active_editor().ok_or_else(|| LexicalError::new("Unable to find an active editor."))?;
    parse_serialized_node_impl(serialized_node, editor.registered_nodes())
}

/// `parseEditorState(serializedEditorState, editor)`. The nodes are created
/// outside any update of the editor, so nothing is marked dirty (JS saves
/// and restores the editor's dirty records instead).
pub(crate) fn parse_editor_state(
    serialized_editor_state: &Value,
    editor: &Editor,
) -> Result<EditorState, LexicalError> {
    let editor_state = create_empty_editor_state();
    let active = Active {
        editor: None,
        editor_state: Some(editor_state.clone()),
        read_only: false,
    };
    with_active(active, || {
        let serialized_node = serialized_editor_state.get("root");
        invariant!(
            serialized_node.is_some_and(|v| !v.is_undefined()),
            "Cannot read properties of undefined (reading 'type')"
        );
        parse_serialized_node_impl(serialized_node.unwrap(), editor.registered_nodes())?;
        Ok(())
    })?;
    // Make the editor state immutable.
    editor_state.0.read_only.set(true);
    editor_state.0.parsed.set(true);
    Ok(editor_state)
}

/// `getEditorStateTextContent(editorState)`.
fn editor_state_text_content(editor_state: &EditorState) -> String {
    editor_state.read(|| get_root().get_text_content())
}

/// `triggerListeners`'s bracket: listeners run with the editor marked as
/// updating, so an update they start is queued.
fn with_updating<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    struct Restore<'a>(&'a Editor, bool);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            self.0.0.updating.set(self.1);
        }
    }
    let _restore = Restore(editor, editor.0.updating.replace(true));
    f()
}

/// `$commitPendingUpdates(editor)`: makes the pending state the editor's
/// state, notifies the text content and update listeners, and runs the
/// first update queued meanwhile.
pub(crate) fn commit_pending_updates(editor: &Editor) -> Result<(), LexicalError> {
    let Some(pending) = editor.0.pending.borrow_mut().take() else {
        return Ok(());
    };
    let current = editor.get_editor_state();
    let needs_update = editor.dirty_type() != DirtyType::None;
    *editor.0.editor_state.borrow_mut() = pending.clone();
    pending.0.read_only.set(true);

    let (dirty_elements, dirty_leaves, normalized_nodes) = {
        let mut dirty = editor.0.dirty.borrow_mut();
        let elements = dirty
            .elements
            .iter()
            .map(|(k, v)| (k.to_owned(), *v))
            .collect();
        let leaves = dirty.leaves.iter().map(str::to_owned).collect();
        let normalized = dirty.normalized.iter().map(str::to_owned).collect();
        if needs_update {
            *dirty = Default::default();
        }
        (elements, leaves, normalized)
    };
    // Reset even when nothing is dirty, so a no-op update's tags don't leak
    // into the next one.
    let tags: Vec<String> = std::mem::take(&mut *editor.0.tags.borrow_mut())
        .iter()
        .map(str::to_owned)
        .collect();

    let previously_dispatching = editor.0.dispatching_commit_listeners.replace(true);
    with_updating(editor, || {
        let listeners = editor.text_content_listeners();
        if !listeners.is_empty() {
            let current_text = editor_state_text_content(&current);
            let latest_text = editor_state_text_content(&pending);
            if current_text != latest_text {
                for listener in &listeners {
                    listener(&latest_text);
                }
            }
        }
        let payload = UpdateListenerPayload {
            dirty_elements,
            dirty_leaves,
            normalized_nodes,
            tags,
            editor_state: pending.clone(),
            prev_editor_state: current.clone(),
        };
        for listener in editor.update_listeners() {
            listener(&payload);
        }
    });
    editor
        .0
        .dispatching_commit_listeners
        .set(previously_dispatching);
    trigger_enqueued_updates(editor)
}

/// `$triggerEnqueuedUpdates(editor)`: begins the next queued update, if
/// any, unless listeners have been queueing updates endlessly.
fn trigger_enqueued_updates(editor: &Editor) -> Result<(), LexicalError> {
    if editor.0.updates.borrow().is_empty() {
        editor.0.cascade_count.set(0);
        return Ok(());
    }
    let count = editor
        .0
        .cascade_count
        .replace(editor.0.cascade_count.get() + 1);
    if count > 99 {
        // JS warns through onWarn.
        editor.0.updates.borrow_mut().clear();
        editor.0.cascade_count.set(0);
        return Ok(());
    }
    let queued = editor.0.updates.borrow_mut().pop_front();
    match queued {
        Some(queued) => begin_update(editor, queued.update_fn, &queued.options),
        None => Ok(()),
    }
}

/// `$processNestedUpdates(editor, initialSkipTransforms)`: runs the updates
/// queued during this one as part of it.
fn process_nested_updates(editor: &Editor, initial_skip: bool) -> Result<bool, LexicalError> {
    let mut skip_transforms = initial_skip;
    loop {
        let queued = editor.0.updates.borrow_mut().pop_front();
        let Some(queued) = queued else { break };
        let pending = editor.pending();
        if queued.options.skip_transforms {
            skip_transforms = true;
        }
        if queued.options.discrete {
            invariant!(
                pending.is_some(),
                "Unexpected empty pending editor state on discrete nested update"
            );
            pending.as_ref().expect("checked").0.flush_sync.set(true);
        }
        editor.add_tags(&queued.options.tags);
        if pending.is_none() {
            begin_update(editor, queued.update_fn, &queued.options)?;
        } else {
            (queued.update_fn)()?;
        }
    }
    Ok(skip_transforms)
}

/// `$discardPendingUpdate(editor, queuedBefore)`: the editor goes back to
/// its committed state; the updates the failed one queued are dropped.
fn discard_pending_update(editor: &Editor, queued_before: &HashSet<u64>) {
    *editor.0.pending.borrow_mut() = Some(editor.get_editor_state());
    {
        let mut dirty = editor.0.dirty.borrow_mut();
        dirty.dirty_type = DirtyType::FullReconcile;
        dirty.leaves = OrderedSet::new();
        dirty.elements = OrderedMap::new();
    }
    editor
        .0
        .updates
        .borrow_mut()
        .retain(|u| queued_before.contains(&u.id));
}

/// `$beginUpdate(editor, updateFn, options)`.
pub(crate) fn begin_update(
    editor: &Editor,
    update_fn: impl FnOnce() -> Result<(), LexicalError>,
    options: &UpdateOptions,
) -> Result<(), LexicalError> {
    editor.add_tags(&options.tags);
    let mut skip_transforms = options.skip_transforms;
    let current = editor.get_editor_state();
    let mut was_cloned = false;
    let pending = match editor.pending() {
        Some(pending) if !pending.0.read_only.get() => pending,
        other => {
            let pending = clone_editor_state(other.as_ref().unwrap_or(&current));
            *editor.0.pending.borrow_mut() = Some(pending.clone());
            was_cloned = true;
            pending
        }
    };
    pending.0.flush_sync.set(options.discrete);
    // The updates queued before this one (see discard_pending_update).
    let queued_before: HashSet<u64> = editor.0.updates.borrow().iter().map(|u| u.id).collect();
    let previous_active_state = ACTIVE.with(|a| a.borrow().editor_state.clone());

    struct Restore<'a>(&'a Editor, bool);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            self.0.0.updating.set(self.1);
            INFINITE_TRANSFORM_COUNT.with(|c| c.set(0));
        }
    }
    let restore = Restore(editor, editor.0.updating.replace(true));

    let active = Active {
        editor: Some(editor.clone()),
        editor_state: Some(pending.clone()),
        read_only: false,
    };
    let result = with_active(active, || {
        let run = || -> Result<(), LexicalError> {
            if was_cloned && let Some(selection) = current.selection() {
                // A headless editor carries the selection over.
                *pending.0.selection.borrow_mut() = Some(selection.clone_selection());
            }
            update_fn()?;
            skip_transforms = process_nested_updates(editor, skip_transforms)?;
            if editor.dirty_type() != DirtyType::None {
                if skip_transforms {
                    normalize_all_dirty_text_nodes(editor)?;
                } else {
                    apply_all_transforms(editor)?;
                }
                process_nested_updates(editor, false)?;
                garbage_collect_detached_nodes(&current, &pending, editor);
            }
            if let Some(selection) = pending.selection() {
                if let Some(range) = selection.range() {
                    let node_map = pending.0.node_map.borrow();
                    invariant!(
                        node_map.contains_key(&range.anchor.key)
                            && node_map.contains_key(&range.focus.key),
                        "updateEditor: selection has been lost because the previously selected nodes have been removed and selection wasn't moved to another node. Ensure selection changes after removing/replacing a selected node."
                    );
                } else if selection.with_node(|n| n.nodes.is_empty()) == Some(true) {
                    *pending.0.selection.borrow_mut() = None;
                }
            }
            Ok(())
        };
        match run() {
            Ok(()) => Ok(()),
            Err(error) => {
                // An update nested in the running update whose pending state
                // it shares leaves the error to that update.
                if !was_cloned
                    && previous_active_state
                        .as_ref()
                        .is_some_and(|s| s.ptr_eq(&pending))
                {
                    return Err(error);
                }
                discard_pending_update(editor, &queued_before);
                // JS reports a failure of this commit to onError too; the
                // caller gets the update's error.
                let _ = commit_pending_updates(editor);
                Err(error)
            }
        }
    });
    drop(restore);
    result?;

    let should_update = editor.dirty_type() != DirtyType::None
        || has_dirty_selection(
            pending.selection().as_ref(),
            editor.get_editor_state().selection().as_ref(),
        );
    if should_update {
        if pending.0.flush_sync.get() {
            pending.0.flush_sync.set(false);
            commit_pending_updates(editor)?;
        }
        // Otherwise the next read or discrete update commits it.
    } else {
        pending.0.flush_sync.set(false);
        if was_cloned {
            *editor.0.tags.borrow_mut() = OrderedSet::new();
            *editor.0.pending.borrow_mut() = None;
        }
    }
    Ok(())
}

/// `updateEditorSync(editor, updateFn, options)`: runs `f` in the editor's
/// running update when there are no options, or in a new update.
pub(crate) fn update_editor_sync(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError>,
    options: Option<&UpdateOptions>,
) -> Result<(), LexicalError> {
    let (in_update, read_only) = ACTIVE.with(|a| {
        let a = a.borrow();
        (
            a.editor.as_ref().is_some_and(|e| e.ptr_eq(editor)),
            a.read_only,
        )
    });
    match options {
        None if in_update && !read_only => f(),
        None => begin_update(editor, f, &UpdateOptions::default()),
        Some(options) => begin_update(editor, f, options),
    }
}

/// `triggerCommandListeners(editor, type, payload)`: the command's
/// listeners from the highest priority down, each priority's in one update,
/// until one returns true.
pub(crate) fn trigger_command_listeners<P: 'static>(
    editor: &Editor,
    command: &LexicalCommand<P>,
    payload: &P,
) -> Result<bool, LexicalError> {
    // A command dispatched from outside an update starts a new action.
    if !editor.0.updating.get() {
        editor.0.cascade_count.set(0);
    }
    for priority in (0..=4).rev() {
        let listeners = editor.command_listeners(command, priority);
        if listeners.is_empty() {
            continue;
        }
        let mut handled = false;
        update_editor_sync(
            editor,
            || {
                for listener in &listeners {
                    if listener(payload, editor)? {
                        handled = true;
                        return Ok(());
                    }
                }
                Ok(())
            },
            None,
        )?;
        if handled {
            return Ok(true);
        }
    }
    Ok(false)
}
