//! Ports of `packages/lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts`.
//!
//! Rust has no `onError` (rust/PARITY.md): an update's error is the
//! `Result` that [`Editor::update`] returns, which is JS's `onError`
//! rethrowing it, so the `onError rethrows` cases are ported, with the
//! errors `onError` saw being the ones returned; the `onError reports`
//! cases are skipped. `await Promise.resolve()`, which commits a
//! non-discrete update in JS, is a read, which commits it in Rust.

use crate::{
    COLLABORATION_TAG, CreateEditorArgs, Editor, EditorState, LexicalError, NodeRef, UpdateOptions,
    create_editor, create_paragraph_node, create_text_node, get_node_by_key, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
struct Commit {
    tags: Vec<String>,
    prev: String,
    next: String,
    dirty_text: Vec<String>,
}

fn text() -> String {
    get_root().get_text_content()
}

fn p(text: &str) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)])?;
    Ok(paragraph)
}

fn text_of(state: &EditorState) -> String {
    state.read(text)
}

fn first() -> Result<NodeRef, LexicalError> {
    get_root()
        .get_first_child_or_throw()?
        .get_first_child_or_throw()
}

fn last() -> Result<NodeRef, LexicalError> {
    get_root()
        .get_last_child_or_throw()?
        .get_first_child_or_throw()
}

fn app_bug() -> LexicalError {
    LexicalError::new("app bug")
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn tagged(discrete: bool, tag: &str) -> UpdateOptions {
    UpdateOptions {
        discrete,
        tags: vec![tag.to_owned()],
        ..UpdateOptions::default()
    }
}

/// `setUp(mode, configure)`, `onError` rethrowing.
fn set_up(configure: impl FnOnce(&Editor)) -> (Editor, Rc<RefCell<Vec<Commit>>>) {
    let editor = create_editor(CreateEditorArgs::default());
    configure(&editor);
    editor
        .update(
            || {
                get_root().append(&[p("one")?, p("two")?])?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let commits = Rc::new(RefCell::new(Vec::new()));
    let log = commits.clone();
    let _ = editor.register_update_listener(move |payload| {
        let state = &payload.editor_state;
        let dirty_text = state.read(|| {
            payload
                .dirty_leaves
                .iter()
                .map(|key| match get_node_by_key(key) {
                    Some(node) if node.is_text() => node.get_text_content(),
                    _ => key.clone(),
                })
                .collect()
        });
        let mut tags = payload.tags.clone();
        tags.sort();
        log.borrow_mut().push(Commit {
            dirty_text,
            next: text_of(state),
            prev: text_of(&payload.prev_editor_state),
            tags,
        });
    });
    (editor, commits)
}

/// `fail('rethrows', run)`: the run throws `app bug`.
fn fail(run: Result<(), LexicalError>) {
    assert_eq!(
        run.map_err(|e| e.message().to_owned()),
        Err("app bug".to_owned())
    );
}

fn expect_next_update_alone(editor: &Editor, commits: &Rc<RefCell<Vec<Commit>>>, before: &str) {
    commits.borrow_mut().clear();
    editor
        .update(
            || {
                last()?.set_text_content("two!");
                Ok(())
            },
            tagged(true, "next"),
        )
        .unwrap();
    let next = format!("{}two!", before.strip_suffix("two").unwrap());
    assert_eq!(
        *commits.borrow(),
        [Commit {
            dirty_text: vec!["two!".to_owned()],
            next,
            prev: before.to_owned(),
            tags: vec!["next".to_owned()],
        }]
    );
}

fn a_callback_that_changes_text_then_throws(is_discrete: bool) {
    let (editor, commits) = set_up(|_| {});
    let result = editor.update(
        || {
            first()?.set_text_content("CHANGED");
            Err(app_bug())
        },
        tagged(is_discrete, "failed"),
    );
    // `errors` (what `onError` saw) is the error the update returned.
    let errors: Vec<&str> = result
        .as_ref()
        .err()
        .map(LexicalError::message)
        .into_iter()
        .collect();
    assert_eq!(errors, ["app bug"]);
    fail(result);
    assert_eq!(text_of(&editor.get_editor_state()), "one\n\ntwo");
    expect_next_update_alone(&editor, &commits, "one\n\ntwo");
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a callback that changes text, then throws (discrete: true)
#[test]
fn a_callback_that_changes_text_then_throws_discrete_true() {
    a_callback_that_changes_text_then_throws(true);
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a callback that changes text, then throws (discrete: false)
#[test]
fn a_callback_that_changes_text_then_throws_discrete_false() {
    a_callback_that_changes_text_then_throws(false);
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a callback that throws before any change, then an update that changes nothing
#[test]
fn a_callback_that_throws_before_any_change_then_an_update_that_changes_nothing() {
    let (editor, commits) = set_up(|_| {});
    fail(editor.update(|| Err(app_bug()), discrete()));
    editor
        .update(|| Ok(()), tagged(true, COLLABORATION_TAG))
        .unwrap();
    expect_next_update_alone(&editor, &commits, "one\n\ntwo");
    assert!(editor.pending().is_none());
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a transform that throws
#[test]
fn a_transform_that_throws() {
    let (editor, commits) = set_up(|e| {
        let _ = e.register_node_transform("text", |node| {
            if node.get_text_content() == "boom" {
                return Err(app_bug());
            }
            Ok(())
        });
    });
    fail(editor.update(
        || {
            first()?.set_text_content("boom");
            Ok(())
        },
        discrete(),
    ));
    assert_eq!(text_of(&editor.get_editor_state()), "one\n\ntwo");
    expect_next_update_alone(&editor, &commits, "one\n\ntwo");
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a nested update that throws discards the update it is part of
#[test]
fn a_nested_update_that_throws_discards_the_update_it_is_part_of() {
    let (editor, commits) = set_up(|_| {});
    let inner = editor.clone();
    fail(editor.update(
        move || {
            first()?.set_text_content("ONE");
            inner.update(
                || {
                    last()?.set_text_content("TWO");
                    Err(app_bug())
                },
                UpdateOptions::default(),
            )
        },
        discrete(),
    ));
    assert_eq!(text_of(&editor.get_editor_state()), "one\n\ntwo");
    expect_next_update_alone(&editor, &commits, "one\n\ntwo");
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > an update batched after another one discards both
#[test]
fn an_update_batched_after_another_one_discards_both() {
    let (editor, commits) = set_up(|_| {});
    editor
        .update(
            || {
                first()?.set_text_content("ONE");
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    fail(editor.update(
        || {
            last()?.set_text_content("TWO");
            Err(app_bug())
        },
        UpdateOptions::default(),
    ));
    // The commit the first update scheduled finds nothing to commit.
    editor.read(|| ()).unwrap();
    assert_eq!(text_of(&editor.get_editor_state()), "one\n\ntwo");
    expect_next_update_alone(&editor, &commits, "one\n\ntwo");
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > what the failed update committed before it threw stays
#[test]
fn what_the_failed_update_committed_before_it_threw_stays() {
    let (editor, commits) = set_up(|_| {});
    let other = editor
        .parse_editor_state_json(&editor.get_editor_state().to_json(false))
        .unwrap();
    let inner = editor.clone();
    fail(editor.update(
        move || {
            first()?.set_text_content("ONE");
            // Commits what this update changed so far; the state it sets
            // is pending.
            inner.set_editor_state(&other, None)?;
            Err(app_bug())
        },
        discrete(),
    ));
    // Its listeners saw 'ONE' committed.
    assert!(commits.borrow().iter().any(|c| c.next == "ONE\n\ntwo"));
    assert_eq!(text_of(&editor.get_editor_state()), "ONE\n\ntwo");
    expect_next_update_alone(&editor, &commits, "ONE\n\ntwo");
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError rethrows > the updates it queued are dropped with it
#[test]
fn the_updates_it_queued_are_dropped_with_it() {
    let (editor, commits) = set_up(|_| {});
    let inner = editor.clone();
    fail(editor.update(
        move || {
            first()?.set_text_content("ONE");
            inner.update(
                || {
                    last()?.set_text_content("TWO");
                    Ok(())
                },
                UpdateOptions::default(),
            )?;
            Err(app_bug())
        },
        discrete(),
    ));
    editor.read(|| ()).unwrap();
    assert_eq!(text_of(&editor.get_editor_state()), "one\n\ntwo");
    expect_next_update_alone(&editor, &commits, "one\n\ntwo");
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError rethrows > the updates queued before it still run
#[test]
fn the_updates_queued_before_it_still_run() {
    let (editor, _) = set_up(|_| {});
    let inner = editor.clone();
    let _ = editor.register_update_listener(move |payload| {
        if payload.tags.iter().any(|t| t == "queue") {
            let _ = inner.update(|| Err(app_bug()), UpdateOptions::default());
            let _ = inner.update(
                || {
                    last()?.set_text_content("TWO");
                    Ok(())
                },
                UpdateOptions::default(),
            );
        }
    });
    fail(editor.update(
        || {
            first()?.set_text_content("ONE");
            Ok(())
        },
        tagged(true, "queue"),
    ));
    let _ = editor.read(|| ());
    assert_eq!(text_of(&editor.get_editor_state()), "ONE\n\nTWO");
}
