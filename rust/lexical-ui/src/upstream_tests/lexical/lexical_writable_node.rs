//! Ports of `packages/lexical/src/__tests__/unit/LexicalWritableNode.test.ts`.
//! A JS node object is a version of a node; here a version is the `Rc` in
//! an editor state's node map, so the identity of a version (`toBe`) is
//! the address of that `Rc`, and a stale JS reference reads the editor
//! state it came from.

use crate::lexical_utils::get_writable;
use crate::upstream_tests::support::panic_message;
use crate::{
    CreateEditorArgs, Editor, EditorState, NodeRef, UpdateOptions, create_editor,
    create_paragraph_node, create_text_node, get_node_by_key, get_root, get_selection,
};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

fn create_headless_editor() -> Editor {
    create_editor(CreateEditorArgs::default())
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// The address of the node's version in the active editor state.
fn version(node: &NodeRef) -> usize {
    Rc::as_ptr(&node.get_latest()) as *const () as usize
}

/// `node.getWritable()`, returning the writable version.
fn writable(node: &NodeRef) -> usize {
    get_writable(node.get_key());
    version(node)
}

/// The address of the node's version in `state`.
fn version_in(state: &EditorState, key: &str) -> usize {
    Rc::as_ptr(&state.0.node_map.borrow()[key]) as *const () as usize
}

fn text_in(state: &EditorState, key: &str) -> String {
    state.0.node_map.borrow()[key].text().unwrap().text.clone()
}

/// What `f` panics with.
fn panics(f: impl FnOnce()) -> String {
    panic_message(&*catch_unwind(AssertUnwindSafe(f)).expect_err("a panic"))
}

const NOT_IN_STATE: &str = "Lexical node does not exist in active editor state";

// upstream: lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > reuses new nodes and clones committed nodes once per update
#[test]
fn reuses_new_nodes_and_clones_committed_nodes_once_per_update() {
    let editor = create_headless_editor();
    let original = Rc::new(RefCell::new(None::<NodeRef>));
    let o = original.clone();
    editor
        .update(
            move || {
                let text = create_text_node("original");
                let created = version(&text);
                assert_eq!(writable(&text), created);
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&text))?;
                get_root().append(&[paragraph])?;
                *o.borrow_mut() = Some(text);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let original = original.borrow().clone().unwrap();
    let key = original.get_key().to_owned();
    let original_state = editor.get_editor_state();
    let original_version = version_in(&original_state, &key);
    let first_clone = Rc::new(RefCell::new(0usize));
    {
        let (original, original_state, first_clone, key) = (
            original.clone(),
            original_state.clone(),
            first_clone.clone(),
            key.clone(),
        );
        editor
            .update(
                move || {
                    original.set_text_content("first");
                    let clone = version(&original);
                    assert_ne!(clone, original_version);
                    assert_eq!(writable(&original), clone);
                    original_state.read(|| {
                        assert_eq!(version(&original), original_version);
                        assert_eq!(original.get_text_content(), "original");
                        assert!(panics(|| get_writable(&key)).contains("read-only"));
                    });
                    assert_eq!(writable(&original), clone);
                    *first_clone.borrow_mut() = clone;
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
    }
    let first_state = editor.get_editor_state();
    let first_clone = *first_clone.borrow();
    {
        let (original, first_state, original_state, key) = (
            original.clone(),
            first_state.clone(),
            original_state.clone(),
            key.clone(),
        );
        editor
            .update(
                move || {
                    original.set_text_content("second");
                    let second_clone = version(&original);
                    assert_ne!(second_clone, first_clone);
                    assert_ne!(second_clone, original_version);
                    assert_eq!(text_in(&first_state, &key), "first");
                    assert_eq!(text_in(&original_state, &key), "original");
                    assert_eq!(writable(&original), second_clone);
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
    }
}

// upstream: lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > reuses live writable nodes across batched updates until commit
#[test]
fn reuses_live_writable_nodes_across_batched_updates_until_commit() {
    let editor = create_headless_editor();
    let original = Rc::new(RefCell::new(None::<NodeRef>));
    let o = original.clone();
    editor
        .update(
            move || {
                let text = create_text_node("original");
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&text))?;
                get_root().append(&[paragraph])?;
                *o.borrow_mut() = Some(text);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let original = original.borrow().clone().unwrap();
    let committed = editor.get_editor_state();
    let key = original.get_key().to_owned();
    let original_version = version_in(&committed, &key);
    let live = Rc::new(RefCell::new(0usize));
    {
        let (original, live) = (original.clone(), live.clone());
        editor
            .update(
                move || {
                    original.set_text_content("first");
                    let w = version(&original);
                    assert_ne!(w, original_version);
                    *live.borrow_mut() = w;
                    Ok(())
                },
                UpdateOptions::default(),
            )
            .unwrap();
    }
    assert!(editor.pending().is_some());
    let live = *live.borrow();
    {
        let (original, committed, key) = (original.clone(), committed.clone(), key.clone());
        editor
            .update(
                move || {
                    assert_eq!(writable(&original), live);
                    original.set_text_content("second");
                    assert_eq!(version(&original), live);
                    assert_eq!(text_in(&committed, &key), "original");
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
    }
    editor
        .update(
            move || {
                assert_ne!(writable(&original), live);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > marks the current parent dirty when writing through a stale reference
#[test]
fn marks_the_current_parent_dirty_when_writing_through_a_stale_reference() {
    let editor = create_headless_editor();
    let keys = Rc::new(RefCell::new((String::new(), String::new(), String::new())));
    let k = keys.clone();
    editor
        .update(
            move || {
                let text = create_text_node("original");
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&text))?;
                let mut keys = k.borrow_mut();
                keys.0 = text.get_key().to_owned();
                keys.1 = paragraph.get_key().to_owned();
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();
    let first_state = editor.get_editor_state();
    let k = keys.clone();
    editor
        .update(
            move || {
                let paragraph = create_paragraph_node();
                k.borrow_mut().2 = paragraph.get_key().to_owned();
                get_root().append(std::slice::from_ref(&paragraph))?;
                paragraph.append(&[NodeRef::new(k.borrow().0.clone())])
            },
            discrete(),
        )
        .unwrap();
    let (text_key, old_parent_key, current_parent_key) = keys.borrow().clone();
    // The reference kept from the first update still names the old parent.
    assert_eq!(
        first_state.0.node_map.borrow()[&text_key]
            .base()
            .parent
            .as_deref(),
        Some(old_parent_key.as_str())
    );
    let e = editor.clone();
    editor
        .update(
            move || {
                NodeRef::new(text_key).set_text_content("changed");
                let dirty = e.0.dirty.borrow();
                assert!(dirty.elements.contains(&current_parent_key));
                assert!(!dirty.elements.contains(&old_parent_key));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > does not reuse a collected subtree in a subsequent batched update
#[test]
fn does_not_reuse_a_collected_subtree_in_a_subsequent_batched_update() {
    let editor = create_headless_editor();
    let text = Rc::new(RefCell::new(None::<NodeRef>));
    let t = text.clone();
    editor
        .update(
            move || {
                let text = create_text_node("detached");
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&text))?;
                get_root().append(std::slice::from_ref(&paragraph))?;
                paragraph.remove(false)?;
                *t.borrow_mut() = Some(text);
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    // GC has run, but the pending state has not committed yet.
    assert!(editor.pending().is_some());
    let text = text.borrow().clone().unwrap();
    editor
        .update(
            move || {
                assert!(get_node_by_key(text.get_key()).is_none());
                assert!(panics(|| get_writable(text.get_key())).contains(NOT_IN_STATE));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > does not reuse a text node collected by normalization
#[test]
fn does_not_reuse_a_text_node_collected_by_normalization() {
    let editor = create_headless_editor();
    let merged = Rc::new(RefCell::new(None::<NodeRef>));
    let m = merged.clone();
    editor
        .update(
            move || {
                let second = create_text_node("second");
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("first"), second.clone()])?;
                get_root().append(&[paragraph])?;
                *m.borrow_mut() = Some(second);
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    let merged = merged.borrow().clone().unwrap();
    editor
        .update(
            move || {
                assert_eq!(get_root().get_text_content(), "firstsecond");
                assert!(get_node_by_key(merged.get_key()).is_none());
                assert!(panics(|| get_writable(merged.get_key())).contains(NOT_IN_STATE));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > restores writable nodes after parsing a separate editor state
#[test]
fn restores_writable_nodes_after_parsing_a_separate_editor_state() {
    let editor = create_headless_editor();
    editor
        .update(
            || {
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("a")])?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();
    let serialized = editor.get_editor_state().to_json(false);
    let e = editor.clone();
    editor
        .update(
            move || {
                let root = writable(&get_root());
                let parsed = e.parse_editor_state_json_with(&serialized, || {
                    assert_ne!(writable(&get_root()), root);
                    let paragraph = create_paragraph_node();
                    paragraph.append(&[create_text_node("b")])?;
                    get_root().append(&[paragraph])
                })?;
                assert_eq!(writable(&get_root()), root);
                assert_eq!(get_root().get_children_size(), 1);
                parsed.read(|| {
                    assert_ne!(version(&get_root()), root);
                    assert_eq!(get_root().get_children_size(), 2);
                });
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > invalidates selection caches even when reusing a writable node
#[test]
fn invalidates_selection_caches_even_when_reusing_a_writable_node() {
    let editor = create_headless_editor();
    editor
        .update(
            || {
                let text = create_text_node("text");
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&text))?;
                get_root().append(&[paragraph])?;
                text.select(Some(0), Some(4))?;
                let selection = get_selection().expect("a selection");
                assert!(selection.with_range(|_| ()).is_some());
                selection.get_nodes()?;
                assert!(selection.with_range(|r| r.cached_nodes.is_some()).unwrap());
                let created = version(&text);
                assert_eq!(writable(&text), created);
                assert!(selection.with_range(|r| r.cached_nodes.is_none()).unwrap());
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalWritableNode.test.ts::stale node references > the suggested guards do not throw for a stale reference
#[test]
fn the_suggested_guards_do_not_throw_for_a_stale_reference() {
    let editor = create_headless_editor();
    let text = Rc::new(RefCell::new(None::<NodeRef>));
    let t = text.clone();
    editor
        .update(
            move || {
                let text = create_text_node("removed");
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&text))?;
                get_root().append(&[paragraph])?;
                *t.borrow_mut() = Some(text);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let text = text.borrow().clone().unwrap();
    let t = text.clone();
    editor
        .update(move || t.get_parent_or_throw()?.remove(false), discrete())
        .unwrap();
    editor
        .update(
            move || {
                assert!(!text.is_attached());
                assert!(get_node_by_key(text.get_key()).is_none());
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}
