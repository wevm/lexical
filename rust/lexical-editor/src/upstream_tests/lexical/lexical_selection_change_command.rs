//! Ports of `packages/lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts`.
//!
//! Upstream mounts its editors on a `contenteditable` in the document; here
//! a [`Root`] stands in for it, whose `is_connected` is false for a root
//! that is not in the document. The DOM text upstream reads is the
//! committed document's text (the reconciler renders it). `onError` is the
//! error an update returns, and the microtask that commits a non-discrete
//! update is the next read.

#![cfg(feature = "extension")]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::selection_insert_nodes::get_previous_selection;
use crate::upstream_tests::support::update;
use crate::{
    COMMAND_PRIORITY_LOW, Editor, LexicalError, LexicalExtension, MutationListenerOptions, NodeRef,
    RangeSelection, RootElement, RootElementRef, SELECTION_CHANGE_COMMAND, SKIP_DOM_SELECTION_TAG,
    Unregister, UpdateOptions, create_node_selection, create_paragraph_node, create_text_node,
    define_extension, get_root, get_selection, on_update, set_selection,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The mounted `contenteditable`: connected while it is in the document.
struct Root {
    connected: Cell<bool>,
}

impl RootElement for Root {
    fn is_connected(&self) -> bool {
        self.connected.get()
    }
}

/// The warnings an editor's `onWarn` heard, which rethrows them when
/// `throws` is set.
#[derive(Default)]
struct Warnings {
    count: Cell<usize>,
    throws: Cell<bool>,
}

/// `buildEditorFromExtensions()`, with its `onWarn` spied on.
fn build_editor() -> (LexicalEditorWithDispose, Rc<Warnings>) {
    let warnings = Rc::new(Warnings::default());
    let w = warnings.clone();
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        on_warn: Some(Rc::new(move |error, _| {
            w.count.set(w.count.get() + 1);
            if w.throws.get() { Err(error) } else { Ok(()) }
        })),
        ..LexicalExtension::<(), (), ()>::new("selection-change-command", ())
    })
    .into()])
    .unwrap();
    (editor, warnings)
}

fn root_element(connected: bool) -> Rc<Root> {
    Rc::new(Root {
        connected: Cell::new(connected),
    })
}

/// `mountEditor(editor)`: a connected root.
fn mount_editor(editor: &Editor) -> Rc<Root> {
    let root = root_element(true);
    editor
        .set_root_element(Some(RootElementRef(root.clone())))
        .unwrap();
    root
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// The `original` paragraph with the caret at 1, which most tests start
/// from.
fn set_up(editor: &Editor, text: &'static str, anchor: usize, focus: usize) {
    update(editor, move || {
        let text = create_text_node(text);
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        text.select(Some(anchor), Some(focus))?;
        Ok(())
    });
}

fn set_up_original(editor: &Editor) {
    set_up(editor, "original", 1, 1);
}

/// `$getRoot().getAllTextNodes()[0]`.
fn first_text() -> NodeRef {
    get_root().get_all_text_nodes()[0].clone()
}

fn range_selection() -> RangeSelection {
    get_selection()
        .and_then(|s| s.range())
        .expect("a RangeSelection")
}

fn committed_text(editor: &Editor) -> String {
    editor.read(|| get_root().get_text_content()).unwrap()
}

/// The text the root element shows: the committed document's.
fn dom_text(editor: &Editor) -> String {
    editor
        .get_editor_state()
        .read(|| get_root().get_text_content())
}

/// Registers `listener` for `SELECTION_CHANGE_COMMAND` at low priority.
fn on_selection_change(
    editor: &Editor,
    listener: impl Fn() -> Result<bool, LexicalError> + 'static,
) -> Unregister {
    editor.register_command(
        &SELECTION_CHANGE_COMMAND,
        move |_, _| listener(),
        COMMAND_PRIORITY_LOW,
    )
}

/// A listener that counts its calls and returns false.
fn counting(editor: &Editor) -> (Rc<Cell<usize>>, Unregister) {
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let unregister = on_selection_change(editor, move || {
        c.set(c.get() + 1);
        Ok(false)
    });
    (calls, unregister)
}

fn select_first_child_node() -> Result<(), LexicalError> {
    let selection = create_node_selection();
    selection.add(get_root().get_first_child_or_throw()?.get_key());
    set_selection(Some(selection));
    Ok(())
}

fn child_texts() -> Vec<String> {
    get_root()
        .get_children()
        .iter()
        .map(NodeRef::get_text_content)
        .collect()
}

fn reconciles_a_same_size_child(replace: bool, normalize: bool) {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    update(&editor, || {
        get_root().clear()?;
        for text in ["first", "second", "third", "fourth"] {
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node(text)])?;
            get_root().append(&[paragraph])?;
        }
        get_root().select_start()?;
        Ok(())
    });
    let previous = editor.get_editor_state();
    let calls = Rc::new(Cell::new(0));
    if normalize {
        let c = calls.clone();
        let _ = on_selection_change(&editor, move || {
            c.set(c.get() + 1);
            let selection = get_selection().filter(|s| s.is_range()).unwrap();
            if !selection.has_format("bold") {
                selection.toggle_format("bold");
            }
            Ok(false)
        });
    }
    update(&editor, move || {
        let root = get_root();
        let third = root.get_child_at_index(2).unwrap();
        if replace {
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("replacement")])?;
            third.replace(&paragraph, false)?;
        } else {
            third.insert_before(&root.get_last_child_or_throw()?, true)?;
        }
        root.select_end()?;
        Ok(())
    });
    let expected = if replace {
        ["first", "second", "replacement", "fourth"]
    } else {
        ["first", "second", "fourth", "third"]
    };
    // The root element's children are the committed document's blocks.
    assert_eq!(
        editor.get_editor_state().read(child_texts),
        expected.map(str::to_owned)
    );
    assert_eq!(
        editor.read(child_texts).unwrap(),
        expected.map(str::to_owned)
    );
    assert_eq!(
        previous.read(child_texts),
        ["first", "second", "third", "fourth"]
    );
    if normalize {
        assert_eq!(calls.get(), 2);
    }
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reconciles a same-size child replace across selection notifications (normalize: false)
#[test]
fn reconciles_a_same_size_child_replace_normalize_false() {
    reconciles_a_same_size_child(true, false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reconciles a same-size child replace across selection notifications (normalize: true)
#[test]
fn reconciles_a_same_size_child_replace_normalize_true() {
    reconciles_a_same_size_child(true, true);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reconciles a same-size child reorder across selection notifications (normalize: false)
#[test]
fn reconciles_a_same_size_child_reorder_normalize_false() {
    reconciles_a_same_size_child(false, false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reconciles a same-size child reorder across selection notifications (normalize: true)
#[test]
fn reconciles_a_same_size_child_reorder_normalize_true() {
    reconciles_a_same_size_child(false, true);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > explicit selection commands retain a fresh update-cascade budget
#[test]
fn explicit_selection_commands_retain_a_fresh_update_cascade_budget() {
    let (editor, warnings) = build_editor();
    let e = editor.editor().clone();
    let _ = on_selection_change(&editor, move || {
        e.update(
            || {
                get_root().mark_dirty();
                Ok(())
            },
            UpdateOptions::default(),
        )?;
        Ok(false)
    });
    let e = editor.editor().clone();
    let unregister = editor.register_update_listener(move |_| {
        e.update(|| Ok(()), UpdateOptions::default()).unwrap();
    });
    for _ in 0..150 {
        // An error here is what upstream's `onError` hears.
        editor
            .dispatch_command(&SELECTION_CHANGE_COMMAND, ())
            .unwrap();
        // The commits upstream flushes with microtasks, without letting the
        // macrotask budget reset run.
        for _ in 0..4 {
            editor.read(|| ()).unwrap();
        }
    }
    unregister();
    assert_eq!(warnings.count.get(), 0);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > does not warn when the last allowed notification clears a rootless selection
#[test]
fn does_not_warn_when_the_last_allowed_notification_clears_a_rootless_selection() {
    let (editor, warnings) = build_editor();
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let _ = on_selection_change(&editor, move || {
        c.set(c.get() + 1);
        if c.get() == 100 {
            set_selection(None);
        } else {
            let root = get_root();
            let selection = create_node_selection();
            let node = if c.get() % 2 == 1 {
                root.get_last_child_or_throw()?
            } else {
                root.get_first_child_or_throw()?
            };
            selection.add(node.get_key());
            set_selection(Some(selection));
        }
        Ok(false)
    });
    update(&editor, || {
        let first = create_paragraph_node();
        get_root().clear()?;
        get_root().append(&[first.clone(), create_paragraph_node()])?;
        let selection = create_node_selection();
        selection.add(first.get_key());
        set_selection(Some(selection));
        Ok(())
    });
    assert_eq!(calls.get(), 100);
    assert_eq!(warnings.count.get(), 0);
    assert!(editor.read(get_selection).unwrap().is_none());
    editor.dispose();
}

fn counts_explicit_dispatches_before_commit(outside: bool) {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    set_up_original(&editor);
    let (calls, _) = counting(&editor);
    let e = editor.editor().clone();
    editor
        .update(
            move || {
                first_text().select(Some(3), Some(3))?;
                if !outside {
                    e.dispatch_command(&SELECTION_CHANGE_COMMAND, ())?;
                }
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    if outside {
        editor
            .dispatch_command(&SELECTION_CHANGE_COMMAND, ())
            .unwrap();
    }
    editor.read(|| ()).unwrap();
    assert_eq!(calls.get(), 1);
    // Explicit commands still run even when the selection is unchanged.
    editor
        .dispatch_command(&SELECTION_CHANGE_COMMAND, ())
        .unwrap();
    editor.read(|| ()).unwrap();
    assert_eq!(calls.get(), 2);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > counts explicit dispatches before commit (outside update: false)
#[test]
fn counts_explicit_dispatches_before_commit_inside_update() {
    counts_explicit_dispatches_before_commit(false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > counts explicit dispatches before commit (outside update: true)
#[test]
fn counts_explicit_dispatches_before_commit_outside_update() {
    counts_explicit_dispatches_before_commit(true);
}

fn aborts_the_update_when_a_selection_listener_throws(explicit: bool) {
    let (editor, warnings) = build_editor();
    mount_editor(&editor);
    set_up_original(&editor);
    let unregister = on_selection_change(&editor, || {
        let text = first_text();
        if text.get_text_content() == "edited!" {
            text.set_text_content("listener change");
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("discard")])?;
            get_root().append(&[paragraph])?;
            get_root().select_end()?;
            return Err(LexicalError::new("selection listener failed"));
        }
        Ok(false)
    });
    let e = editor.editor().clone();
    let result = editor.update(
        move || {
            let text = first_text();
            text.set_text_content("edited!");
            text.select(Some(3), Some(3))?;
            if explicit {
                e.dispatch_command(&SELECTION_CHANGE_COMMAND, ())?;
            }
            Ok(())
        },
        discrete(),
    );
    // `onError` hears the failure once.
    assert_eq!(
        result.map_err(|e| e.message().to_owned()),
        Err("selection listener failed".to_owned())
    );
    assert_eq!(warnings.count.get(), 0);
    assert_eq!(dom_text(&editor), "original");
    editor
        .read(|| {
            assert_eq!(get_root().get_text_content(), "original");
            assert_eq!(range_selection().anchor.offset, 1);
        })
        .unwrap();
    unregister();
    editor
        .update(
            || {
                first_text().select(Some(4), Some(4))?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > aborts the update when a selection listener throws (explicit dispatch: false)
#[test]
fn aborts_the_update_when_a_selection_listener_throws_implicit() {
    aborts_the_update_when_a_selection_listener_throws(false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > aborts the update when a selection listener throws (explicit dispatch: true)
#[test]
fn aborts_the_update_when_a_selection_listener_throws_explicit() {
    aborts_the_update_when_a_selection_listener_throws(true);
}

fn restores_the_notification_baseline_after_rollback(explicit: bool, retry_selection: bool) {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    set_up_original(&editor);
    let calls = Rc::new(Cell::new(0));
    // `vi.fn(() => false).mockImplementationOnce(() => { throw failure })`.
    let thrown = Rc::new(Cell::new(false));
    let c = calls.clone();
    let _ = on_selection_change(&editor, move || {
        c.set(c.get() + 1);
        if !thrown.replace(true) {
            return Err(LexicalError::new("selection listener failed"));
        }
        Ok(false)
    });
    let e = editor.editor().clone();
    let result = editor.update(
        move || {
            first_text().select(Some(3), Some(3))?;
            if explicit {
                e.dispatch_command(&SELECTION_CHANGE_COMMAND, ())?;
            }
            Ok(())
        },
        discrete(),
    );
    assert_eq!(
        result.map_err(|e| e.message().to_owned()),
        Err("selection listener failed".to_owned())
    );
    assert_eq!(calls.get(), 1);
    calls.set(0);
    editor
        .update(
            move || {
                if retry_selection {
                    first_text().select(Some(3), Some(3))?;
                }
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                on_update: Some(Rc::new(|| {})),
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    assert_eq!(calls.get(), usize::from(retry_selection));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > restores the notification baseline after rollback (explicit: false, retry selection: false)
#[test]
fn restores_the_notification_baseline_after_rollback_implicit_no_retry() {
    restores_the_notification_baseline_after_rollback(false, false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > restores the notification baseline after rollback (explicit: false, retry selection: true)
#[test]
fn restores_the_notification_baseline_after_rollback_implicit_retry() {
    restores_the_notification_baseline_after_rollback(false, true);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > restores the notification baseline after rollback (explicit: true, retry selection: false)
#[test]
fn restores_the_notification_baseline_after_rollback_explicit_no_retry() {
    restores_the_notification_baseline_after_rollback(true, false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > restores the notification baseline after rollback (explicit: true, retry selection: true)
#[test]
fn restores_the_notification_baseline_after_rollback_explicit_retry() {
    restores_the_notification_baseline_after_rollback(true, true);
}

fn aborts_the_update_when_listener_work_fails(nested_update: bool) {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    set_up_original(&editor);
    let _ = editor.register_node_transform("text", |node| {
        if node.get_text_content() == "transform failure" {
            node.set_text_content("must be discarded");
            return Err(LexicalError::new("listener work failed"));
        }
        Ok(())
    });
    editor.read(|| ()).unwrap();
    let e = editor.editor().clone();
    let _ = on_selection_change(&editor, move || {
        let text = first_text();
        text.set_text_content("transform failure");
        if nested_update {
            e.update(
                move || {
                    text.remove(false)?;
                    Err(LexicalError::new("listener work failed"))
                },
                UpdateOptions::default(),
            )?;
        }
        Ok(false)
    });
    let result = editor.update(
        || {
            let text = first_text();
            text.set_text_content("edited!");
            text.select(Some(3), Some(3))?;
            Ok(())
        },
        discrete(),
    );
    assert_eq!(
        result.map_err(|e| e.message().to_owned()),
        Err("listener work failed".to_owned())
    );
    assert_eq!(committed_text(&editor), "original");
    assert_eq!(dom_text(&editor), "original");
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > aborts the update when listener work fails in a transform
#[test]
fn aborts_the_update_when_listener_work_fails_in_a_transform() {
    aborts_the_update_when_listener_work_fails(false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > aborts the update when listener work fails in a nested update
#[test]
fn aborts_the_update_when_listener_work_fails_in_a_nested_update() {
    aborts_the_update_when_listener_work_fails(true);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > does not replay skipped range changes when a detached root reconnects
#[test]
fn does_not_replay_skipped_range_changes_when_a_detached_root_reconnects() {
    let (editor, _) = build_editor();
    let root = mount_editor(&editor);
    set_up_original(&editor);
    root.connected.set(false);
    let (calls, _) = counting(&editor);
    update(&editor, || {
        first_text().select(Some(3), Some(3))?;
        Ok(())
    });
    assert_eq!(calls.get(), 0);
    root.connected.set(true);
    editor
        .update(
            || get_root().append(&[create_paragraph_node()]),
            UpdateOptions {
                discrete: true,
                tags: vec![SKIP_DOM_SELECTION_TAG.to_owned()],
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    assert_eq!(calls.get(), 0);
    editor.dispose();
}

fn caps_notifications_without_discarding_edits(throws: bool) {
    let (editor, warnings) = build_editor();
    mount_editor(&editor);
    set_up_original(&editor);
    warnings.throws.set(throws);
    let updates = Rc::new(Cell::new(0));
    let u = updates.clone();
    let _ = editor.register_update_listener(move |_| u.set(u.get() + 1));
    let on_update_calls = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let unregister = on_selection_change(&editor, move || {
        c.set(c.get() + 1);
        let offset = if range_selection().anchor.offset == 1 {
            2
        } else {
            1
        };
        first_text().select(Some(offset), Some(offset))?;
        Ok(false)
    });
    // A discrete commit gives the caller a throwing onWarn's error.
    let o = on_update_calls.clone();
    let result = editor.update(
        || {
            let text = first_text();
            text.set_text_content("TYPED original");
            text.select(Some(2), Some(2))?;
            Ok(())
        },
        UpdateOptions {
            discrete: true,
            on_update: Some(Rc::new(move || o.set(o.get() + 1))),
            ..UpdateOptions::default()
        },
    );
    if throws {
        assert_eq!(
            result.map_err(|e| e.message().to_owned()),
            Err("Selection change listeners are endlessly changing the selection.".to_owned())
        );
    } else {
        result.unwrap();
    }
    assert_eq!(calls.get(), 100);
    assert_eq!(warnings.count.get(), 1);
    assert_eq!(committed_text(&editor), "TYPED original");
    assert_eq!(dom_text(&editor), "TYPED original");
    assert_eq!(updates.get(), 1);
    assert_eq!(on_update_calls.get(), 1);
    unregister();
    update(&editor, || {
        first_text().select(Some(3), Some(3))?;
        Ok(())
    });
    editor
        .read(|| assert_eq!(range_selection().anchor.offset, 3))
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > caps notifications without discarding edits (throwing warning: false)
#[test]
fn caps_notifications_without_discarding_edits_non_throwing_warning() {
    caps_notifications_without_discarding_edits(false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > caps notifications without discarding edits (throwing warning: true)
#[test]
fn caps_notifications_without_discarding_edits_throwing_warning() {
    caps_notifications_without_discarding_edits(true);
}

#[derive(Clone, Copy, PartialEq)]
enum RootKind {
    Mounted,
    Rootless,
    Detached,
}

fn node_selections_notify_before_commit(kind: RootKind) {
    let (editor, _) = build_editor();
    match kind {
        RootKind::Mounted => {
            mount_editor(&editor);
        }
        RootKind::Detached => editor
            .set_root_element(Some(RootElementRef(root_element(false))))
            .unwrap(),
        RootKind::Rootless => {}
    }
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("original")])?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    let previous = editor.get_editor_state();
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let e = editor.editor().clone();
    let _ = on_selection_change(&editor, move || {
        c.set(c.get() + 1);
        assert!(e.get_editor_state().ptr_eq(&previous));
        let previous_selection = previous.selection();
        match (get_previous_selection(), previous_selection) {
            (None, None) => {}
            (Some(a), Some(b)) => assert!(a.ptr_eq(&b)),
            _ => panic!("$getPreviousSelection() is not the previous selection"),
        }
        assert_eq!(get_root().get_text_content(), "CHANGED");
        if kind == RootKind::Mounted {
            assert_eq!(dom_text(&e), "original");
        }
        Ok(false)
    });
    update(&editor, || {
        let text = first_text();
        text.set_text_content("CHANGED");
        let selection = create_node_selection();
        selection.add(text.get_key());
        set_selection(Some(selection));
        Ok(())
    });
    assert_eq!(calls.get(), 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > node selections notify before commit in a mounted editor
#[test]
fn node_selections_notify_before_commit_in_a_mounted_editor() {
    node_selections_notify_before_commit(RootKind::Mounted);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > node selections notify before commit in a rootless editor
#[test]
fn node_selections_notify_before_commit_in_a_rootless_editor() {
    node_selections_notify_before_commit(RootKind::Rootless);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > node selections notify before commit in a detached editor
#[test]
fn node_selections_notify_before_commit_in_a_detached_editor() {
    node_selections_notify_before_commit(RootKind::Detached);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > listener edits and transforms are reconciled in the original commit
#[test]
fn listener_edits_and_transforms_are_reconciled_in_the_original_commit() {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    set_up(&editor, "Hello", 1, 1);
    let updates = Rc::new(Cell::new(0));
    let u = updates.clone();
    let _ = editor.register_update_listener(move |_| u.set(u.get() + 1));
    let _ = editor.register_node_transform("text", |node| {
        if node.get_text_content() == "Changed" {
            node.set_text_content("Transformed");
        }
        Ok(())
    });
    editor.read(|| ()).unwrap();
    updates.set(0);
    let previous_state = editor.get_editor_state();
    let e = editor.editor().clone();
    let unregister = on_selection_change(&editor, move || {
        assert!(e.get_editor_state().ptr_eq(&previous_state));
        assert_eq!(dom_text(&e), "Hello");
        range_selection();
        assert!(
            get_previous_selection()
                .unwrap()
                .ptr_eq(&previous_state.selection().unwrap())
        );
        first_text().set_text_content("Changed");
        Ok(false)
    });
    update(&editor, || {
        first_text().select(Some(2), Some(2))?;
        Ok(())
    });
    unregister();
    assert_eq!(committed_text(&editor), "Transformed");
    assert_eq!(dom_text(&editor), "Transformed");
    assert_eq!(updates.get(), 1);
    editor.dispose();
}

#[cfg(feature = "history")]
#[derive(Clone, Copy, PartialEq)]
enum Callback {
    OnUpdate,
    DollarOnUpdate,
    SelectionListener,
    None,
}

#[cfg(feature = "history")]
fn preserves_typing_undo_merging(callback: Callback) {
    use crate::history::{HistoryConfigOverrides, HistoryNow, history_extension};
    use crate::{CAN_UNDO_COMMAND, UNDO_COMMAND, config_extension};
    let editor = build_editor_from_extensions([config_extension(
        &history_extension(),
        [HistoryConfigOverrides {
            delay: Some(1000.0),
            now: Some(HistoryNow(Rc::new(|| 0.0))),
            ..HistoryConfigOverrides::default()
        }],
    )])
    .unwrap();
    mount_editor(&editor);
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?;
        Ok(())
    });
    let on_update_calls = Rc::new(Cell::new(0));
    let updates = Rc::new(Cell::new(0));
    // Toolbars listen to history availability during the commit, while
    // callbacks from selection listeners are still waiting to run.
    let _ = editor.register_command(&CAN_UNDO_COMMAND, |_, _| Ok(false), COMMAND_PRIORITY_LOW);
    let o = on_update_calls.clone();
    let unregister_selection = on_selection_change(&editor, move || {
        if callback == Callback::SelectionListener {
            let o = o.clone();
            on_update(move || o.set(o.get() + 1));
        }
        Ok(false)
    });
    let u = updates.clone();
    let unregister = editor.register_update_listener(move |_| u.set(u.get() + 1));
    for character in ["a", "b", "c", "d"] {
        let o = on_update_calls.clone();
        let o2 = on_update_calls.clone();
        editor
            .update(
                move || {
                    range_selection();
                    get_selection().unwrap().insert_text(character)?;
                    if callback == Callback::DollarOnUpdate {
                        on_update(move || o.set(o.get() + 1));
                    }
                    Ok(())
                },
                UpdateOptions {
                    discrete: true,
                    on_update: (callback == Callback::OnUpdate)
                        .then(|| Rc::new(move || o2.set(o2.get() + 1)) as Rc<dyn Fn()>),
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
        editor.read(|| ()).unwrap();
    }
    assert_eq!(committed_text(&editor), "abcd");
    unregister();
    unregister_selection();
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(committed_text(&editor), "");
    assert_eq!(updates.get(), 4);
    assert_eq!(
        on_update_calls.get(),
        if callback == Callback::None { 0 } else { 4 }
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves typing undo merging with onUpdate callbacks
#[cfg(feature = "history")]
#[test]
fn preserves_typing_undo_merging_with_on_update_callbacks() {
    preserves_typing_undo_merging(Callback::OnUpdate);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves typing undo merging with $onUpdate callbacks
#[cfg(feature = "history")]
#[test]
fn preserves_typing_undo_merging_with_dollar_on_update_callbacks() {
    preserves_typing_undo_merging(Callback::DollarOnUpdate);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves typing undo merging with selection listener callbacks
#[cfg(feature = "history")]
#[test]
fn preserves_typing_undo_merging_with_selection_listener_callbacks() {
    preserves_typing_undo_merging(Callback::SelectionListener);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves typing undo merging with none callbacks
#[cfg(feature = "history")]
#[test]
fn preserves_typing_undo_merging_with_none_callbacks() {
    preserves_typing_undo_merging(Callback::None);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves callbacks added by a command during commit listeners
#[test]
fn preserves_callbacks_added_by_a_command_during_commit_listeners() {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("initial")])?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    let command = crate::create_command::<()>("command");
    let callbacks = Rc::new(RefCell::new(Vec::<String>::new()));
    let (c, e) = (callbacks.clone(), editor.editor().clone());
    let _ = editor.register_command(
        &command,
        move |_, _| {
            first_text().set_text_content("second");
            let (c, e) = (c.clone(), e.clone());
            on_update(move || c.borrow_mut().push(format!("second: {}", dom_text(&e))));
            Ok(true)
        },
        COMMAND_PRIORITY_LOW,
    );
    let slot: Rc<RefCell<Option<Unregister>>> = Rc::default();
    let (s, e) = (slot.clone(), editor.editor().clone());
    *slot.borrow_mut() = Some(editor.register_update_listener(move |_| {
        if let Some(unregister) = s.borrow_mut().take() {
            unregister();
        }
        e.dispatch_command(&command, ()).unwrap();
    }));
    let (c, e) = (callbacks.clone(), editor.editor().clone());
    update(&editor, move || {
        first_text().set_text_content("first");
        on_update(move || c.borrow_mut().push(format!("first: {}", dom_text(&e))));
        Ok(())
    });
    // `await Promise.resolve()`: the command's update commits.
    editor.read(|| ()).unwrap();
    assert_eq!(*callbacks.borrow(), ["first: first", "second: second"]);
    editor.dispose();
}

fn preserves_existing_notification_behavior(detached: bool) {
    let (editor, _) = build_editor();
    if detached {
        let root = root_element(false);
        editor
            .set_root_element(Some(RootElementRef(root.clone())))
            .unwrap();
        assert!(!root.is_connected());
    }
    let (calls, _) = counting(&editor);
    set_up(&editor, "Hello world", 2, 5);
    update(&editor, || {
        set_selection(None);
        Ok(())
    });
    assert_eq!(calls.get(), 0);
    // Non-range selection notifications already work without a connected
    // root.
    update(&editor, select_first_child_node);
    assert_eq!(calls.get(), 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves existing notification behavior for a rootless editor
#[test]
fn preserves_existing_notification_behavior_for_a_rootless_editor() {
    preserves_existing_notification_behavior(false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves existing notification behavior for a detached editor
#[test]
fn preserves_existing_notification_behavior_for_a_detached_editor() {
    preserves_existing_notification_behavior(true);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > tracks non-range notifications while the root is detached
#[test]
fn tracks_non_range_notifications_while_the_root_is_detached() {
    let (editor, _) = build_editor();
    let root = mount_editor(&editor);
    set_up(&editor, "Hello world", 2, 5);
    let (calls, _) = counting(&editor);
    root.connected.set(false);
    update(&editor, select_first_child_node);
    assert_eq!(calls.get(), 1);
    root.connected.set(true);
    update(&editor, || {
        first_text().select(Some(2), Some(5))?;
        Ok(())
    });
    assert_eq!(calls.get(), 2);
    editor.dispose();
}

#[derive(Clone, Copy, PartialEq)]
enum Change {
    Move,
    Clear,
    Replace,
    ContentOnly,
}

fn preserves_mutation_listener_updates(change: Change, discrete_update: bool) {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    set_up(&editor, "Hello world", 1, 1);
    let selections = Rc::new(RefCell::new(Vec::<Option<crate::SelectionRef>>::new()));
    let s = selections.clone();
    let unregister_command = on_selection_change(&editor, move || {
        s.borrow_mut()
            .push(get_selection().map(|s| s.clone_selection()));
        Ok(false)
    });
    let slot: Rc<RefCell<Option<Unregister>>> = Rc::default();
    let (sl, e) = (slot.clone(), editor.editor().clone());
    *slot.borrow_mut() = Some(
        editor
            .register_mutation_listener(
                "text",
                move |_, _| {
                    if let Some(unregister) = sl.borrow_mut().take() {
                        unregister();
                    }
                    e.update(
                        move || {
                            match change {
                                Change::Clear => set_selection(None),
                                Change::Replace => {
                                    let text = create_text_node("Replacement");
                                    let paragraph = create_paragraph_node();
                                    paragraph.append(std::slice::from_ref(&text))?;
                                    get_root().clear()?;
                                    get_root().append(&[paragraph])?;
                                    text.select(Some(7), Some(7))?;
                                }
                                Change::Move => {
                                    first_text().select(Some(7), Some(7))?;
                                }
                                Change::ContentOnly => {
                                    first_text().set_text_content("Hello world!?");
                                }
                            }
                            Ok(())
                        },
                        if discrete_update {
                            discrete()
                        } else {
                            UpdateOptions::default()
                        },
                    )
                    .unwrap();
                },
                MutationListenerOptions {
                    skip_initialization: true,
                },
            )
            .unwrap(),
    );
    update(&editor, || {
        let text = first_text();
        text.set_text_content("Hello world!");
        text.select(Some(2), Some(2))?;
        Ok(())
    });
    editor
        .read(|| {
            let selection = get_selection();
            match change {
                Change::Clear => assert!(selection.is_none()),
                _ => {
                    let range = range_selection();
                    let offset = if change == Change::ContentOnly { 2 } else { 7 };
                    assert_eq!((range.anchor.offset, range.focus.offset), (offset, offset));
                }
            }
            assert_eq!(
                get_root().get_text_content(),
                match change {
                    Change::Replace => "Replacement",
                    Change::ContentOnly => "Hello world!?",
                    _ => "Hello world!",
                }
            );
            // Mutation listeners run after the first selection has
            // committed. Their changes belong to a second update and notify
            // before its commit.
            let selections = selections.borrow();
            assert_eq!(
                selections.len(),
                if change == Change::ContentOnly { 1 } else { 2 }
            );
            let last = selections.last().unwrap();
            match (&selection, last) {
                (None, None) => {}
                (Some(a), Some(b)) => assert!(a.get().is(&b.get())),
                _ => panic!("the last notified selection is not the committed one"),
            }
        })
        .unwrap();
    unregister_command();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: move (discrete: false)
#[test]
fn preserves_mutation_listener_updates_move() {
    preserves_mutation_listener_updates(Change::Move, false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: clear (discrete: false)
#[test]
fn preserves_mutation_listener_updates_clear() {
    preserves_mutation_listener_updates(Change::Clear, false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: replace (discrete: false)
#[test]
fn preserves_mutation_listener_updates_replace() {
    preserves_mutation_listener_updates(Change::Replace, false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: content only (discrete: false)
#[test]
fn preserves_mutation_listener_updates_content_only() {
    preserves_mutation_listener_updates(Change::ContentOnly, false);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: move (discrete: true)
#[test]
fn preserves_mutation_listener_updates_move_discrete() {
    preserves_mutation_listener_updates(Change::Move, true);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: clear (discrete: true)
#[test]
fn preserves_mutation_listener_updates_clear_discrete() {
    preserves_mutation_listener_updates(Change::Clear, true);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: replace (discrete: true)
#[test]
fn preserves_mutation_listener_updates_replace_discrete() {
    preserves_mutation_listener_updates(Change::Replace, true);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: content only (discrete: true)
#[test]
fn preserves_mutation_listener_updates_content_only_discrete() {
    preserves_mutation_listener_updates(Change::ContentOnly, true);
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reports committed selection changes without requiring DOM events
#[test]
fn reports_committed_selection_changes_without_requiring_dom_events() {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    update(&editor, || {
        let text = create_text_node("Hello world");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().append(&[paragraph])?;
        text.select(Some(2), Some(9))?;
        Ok(())
    });
    let (calls, _) = counting(&editor);
    let selection = || get_selection().filter(|s| s.is_range()).unwrap();
    update(&editor, move || selection().remove_text());
    assert_eq!(calls.get(), 1);
    update(&editor, move || selection().insert_text("y"));
    assert_eq!(calls.get(), 2);
    update(&editor, || get_root().select_end().map(|_| ()));
    assert_eq!(calls.get(), 3);
    update(&editor, move || {
        selection().toggle_format("bold");
        Ok(())
    });
    assert_eq!(calls.get(), 4);
    update(&editor, move || {
        selection().set_style("color: red");
        Ok(())
    });
    assert_eq!(calls.get(), 5);
    // Dirtying the same selection or changing content without moving it
    // is not a selection change.
    update(&editor, || {
        set_selection(Some(get_selection().unwrap().clone_selection()));
        Ok(())
    });
    update(&editor, || get_root().append(&[create_paragraph_node()]));
    assert_eq!(calls.get(), 5);
    update(&editor, select_first_child_node);
    assert_eq!(calls.get(), 6);
    update(&editor, || {
        set_selection(None);
        Ok(())
    });
    assert_eq!(calls.get(), 7);
    update(&editor, || {
        set_selection(None);
        Ok(())
    });
    assert_eq!(calls.get(), 7);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reports selection changes from setEditorState
#[test]
fn reports_selection_changes_from_set_editor_state() {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    update(&editor, || {
        let text = create_text_node("Hello world");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().append(&[paragraph])?;
        text.select(Some(2), Some(9))?;
        Ok(())
    });
    let previous_state = editor.get_editor_state();
    update(&editor, || get_root().select_end().map(|_| ()));
    let (calls, _) = counting(&editor);
    editor.set_editor_state(&previous_state, None).unwrap();
    editor.read(|| ()).unwrap();
    assert_eq!(calls.get(), 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > listeners see the committed selection when DOM synchronization is skipped
#[test]
fn listeners_see_the_committed_selection_when_dom_synchronization_is_skipped() {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    set_up(&editor, "Hello world", 1, 1);
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let unregister = on_selection_change(&editor, move || {
        c.set(c.get() + 1);
        let selection = range_selection();
        assert_eq!((selection.anchor.offset, selection.focus.offset), (3, 7));
        let previous = get_previous_selection().and_then(|s| s.range()).unwrap();
        assert_eq!((previous.anchor.offset, previous.focus.offset), (1, 1));
        Ok(false)
    });
    editor
        .update(
            || {
                first_text().select(Some(3), Some(7))?;
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                tags: vec![SKIP_DOM_SELECTION_TAG.to_owned()],
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    assert_eq!(calls.get(), 1);
    unregister();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > notifies a selection change made by a command listener
#[test]
fn notifies_a_selection_change_made_by_a_command_listener() {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    update(&editor, || {
        let text = create_text_node("Hello world");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().append(&[paragraph])?;
        text.select(Some(1), Some(1))?;
        Ok(())
    });
    let offsets = Rc::new(RefCell::new(Vec::<[usize; 2]>::new()));
    let o = offsets.clone();
    let _ = on_selection_change(&editor, move || {
        let selection = range_selection();
        let previous = get_previous_selection().and_then(|s| s.range()).unwrap();
        o.borrow_mut()
            .push([previous.anchor.offset, selection.anchor.offset]);
        if selection.anchor.offset == 3 {
            first_text().select(Some(4), Some(4))?;
        }
        Ok(false)
    });
    update(&editor, || {
        first_text().select(Some(3), Some(3))?;
        Ok(())
    });
    editor.read(|| ()).unwrap();
    assert_eq!(*offsets.borrow(), [[1, 3], [1, 4]]);
    editor
        .read(|| {
            let selection = get_selection().unwrap().get();
            let previous = get_previous_selection().unwrap().get();
            assert!(selection.is(&previous));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > notifies once for batched changes and skips a batch with no net change
#[test]
fn notifies_once_for_batched_changes_and_skips_a_batch_with_no_net_change() {
    let (editor, _) = build_editor();
    mount_editor(&editor);
    update(&editor, || {
        let text = create_text_node("Hello world");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().append(&[paragraph])?;
        text.select(Some(1), Some(1))?;
        Ok(())
    });
    let (calls, _) = counting(&editor);
    let select = |offset: usize| {
        editor
            .update(
                move || {
                    first_text().select(Some(offset), Some(offset))?;
                    Ok(())
                },
                UpdateOptions::default(),
            )
            .unwrap();
    };
    select(2);
    select(3);
    editor.read(|| ()).unwrap();
    assert_eq!(calls.get(), 1);
    select(4);
    select(3);
    editor.read(|| ()).unwrap();
    assert_eq!(calls.get(), 1);
    editor.dispose();
}
