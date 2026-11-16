//! Ports of `packages/lexical/src/__tests__/unit/LexicalEditorState.test.ts`,
//! whose editor `initializeUnitTest` mounts on a root element.

use crate::editor_state::{EditorState, NodeMap, create_empty_editor_state};
use crate::json::parse;
use crate::node::LexicalNode;
use crate::nodes::root::RootNode;
use crate::updates::read_editor_state;
use crate::upstream_tests::support::{json_to_equal, panic_message};
use crate::{
    CreateEditorArgs, Editor, EditorReadMode, RootElement, RootElementRef, UpdateOptions,
    create_editor, create_paragraph_node, create_text_node, get_editor, get_root, reset_random_key,
};
use std::rc::Rc;

struct TestRoot;
impl RootElement for TestRoot {}

/// `initializeUnitTest`'s editor: keys from 1, mounted on a root element.
fn mounted_editor() -> Editor {
    reset_random_key();
    let editor = create_editor(CreateEditorArgs {
        namespace: "test".to_owned(),
        ..CreateEditorArgs::default()
    });
    editor
        .set_root_element(Some(RootElementRef(Rc::new(TestRoot))))
        .unwrap();
    editor
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// `new EditorState(nodeMap)`.
fn new_editor_state(node_map: NodeMap) -> EditorState {
    let state = create_empty_editor_state();
    *state.0.node_map.borrow_mut() = node_map;
    state
}

fn node_map_with_root(root: Rc<dyn LexicalNode>) -> NodeMap {
    let mut node_map = NodeMap::new();
    node_map.insert("root".to_owned(), root);
    node_map
}

// upstream: lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > isEmpty
#[test]
fn is_empty() {
    let _editor = mounted_editor();
    let root: Rc<dyn LexicalNode> = Rc::new(RootNode::new());
    assert!(new_editor_state(node_map_with_root(root)).is_empty());
    // A state with nothing in it at all is emptier still.
    assert!(new_editor_state(NodeMap::new()).is_empty());
}

// upstream: lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > constructor
#[test]
fn constructor() {
    let _editor = mounted_editor();
    let root: Rc<dyn LexicalNode> = Rc::new(RootNode::new());
    let editor_state: EditorState = new_editor_state(node_map_with_root(root.clone()));
    // The state holds the node map it was given: the same root node.
    let node_map = editor_state.0.node_map.borrow();
    assert_eq!(node_map.len(), 1);
    assert!(Rc::ptr_eq(&node_map["root"], &root));
    assert!(editor_state.selection().is_none());
}

// upstream: lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > read('pending' | 'latest' | 'force-commit')
#[test]
fn read_pending_latest_force_commit() {
    let editor = mounted_editor();
    let text_content = || get_root().get_text_content();
    // A regular update is processed synchronously but committed later.
    editor
        .update(
            || {
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("foo")])?;
                get_root().append(&[paragraph])
            },
            UpdateOptions::default(),
        )
        .unwrap();
    assert_eq!(editor.get_editor_state().read(text_content), "");
    // read('pending') observes the pending state without flushing it.
    assert_eq!(
        editor
            .read_with_mode(EditorReadMode::Pending, text_content)
            .unwrap(),
        "foo"
    );
    // read('latest') reads the committed state without flushing.
    assert_eq!(
        editor
            .read_with_mode(EditorReadMode::Latest, text_content)
            .unwrap(),
        ""
    );
    assert_eq!(editor.get_editor_state().read(text_content), "");
    // read() defaults to 'force-commit', which flushes the pending state.
    assert_eq!(editor.read(text_content).unwrap(), "foo");
    assert_eq!(editor.get_editor_state().read(text_content), "foo");
    // Without a pending state, all modes read the committed state.
    for mode in [
        EditorReadMode::Pending,
        EditorReadMode::Latest,
        EditorReadMode::ForceCommit,
    ] {
        assert_eq!(editor.read_with_mode(mode, text_content).unwrap(), "foo");
    }
}

// upstream: lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > read()
#[test]
fn read() {
    let editor = mounted_editor();
    editor
        .update(
            || {
                let paragraph = create_paragraph_node();
                let text = create_text_node("foo");
                paragraph.append(&[text])?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();

    let (root, paragraph, text) = editor.get_editor_state().read(|| {
        let root = get_root();
        let paragraph = root.get_first_child().expect("a paragraph");
        assert_eq!(paragraph.get_type(), "paragraph");
        let text = paragraph.get_first_child().expect("a text node");
        assert!(text.is_text());
        (root.get_latest(), paragraph.get_latest(), text.get_latest())
    });

    // The root caches its text content when it reconciles (`__cachedText`).
    {
        let cache = editor.0.root_text_cache.borrow();
        let (cached, cached_text) = cache.as_ref().expect("a cached text");
        assert!(Rc::ptr_eq(cached, &root));
        assert_eq!(cached_text, "foo");
    }
    let (b, e) = (root.base(), root.element().unwrap());
    assert_eq!(
        (
            b.key.as_str(),
            &b.parent,
            &b.prev,
            &b.next,
            &b.slot_host,
            b.slots.is_none()
        ),
        ("root", &None, &None, &None, &None, true)
    );
    assert_eq!(
        (e.dir, e.format, e.indent, e.text_format),
        (None, 0, 0.0, 0.0)
    );
    assert_eq!(
        (e.first.as_deref(), e.last.as_deref(), e.size),
        (Some("1"), Some("1"), 1)
    );
    assert_eq!((e.style.as_str(), e.text_style.as_str()), ("", ""));
    assert_eq!(root.get_type(), "root");

    let (b, e) = (paragraph.base(), paragraph.element().unwrap());
    assert_eq!(
        (
            b.key.as_str(),
            b.parent.as_deref(),
            &b.prev,
            &b.next,
            &b.slot_host,
            b.slots.is_none()
        ),
        ("1", Some("root"), &None, &None, &None, true)
    );
    assert_eq!(
        (e.dir, e.format, e.indent, e.text_format),
        (None, 0, 0.0, 0.0)
    );
    assert_eq!(
        (e.first.as_deref(), e.last.as_deref(), e.size),
        (Some("2"), Some("2"), 1)
    );
    assert_eq!((e.style.as_str(), e.text_style.as_str()), ("", ""));
    assert_eq!(paragraph.get_type(), "paragraph");

    let (b, t) = (text.base(), text.text().unwrap());
    assert_eq!(
        (b.key.as_str(), b.parent.as_deref(), &b.prev, &b.next),
        ("2", Some("1"), &None, &None)
    );
    assert_eq!(
        (
            t.detail,
            t.format,
            t.mode,
            t.style.as_str(),
            t.text.as_str()
        ),
        (0.0, 0.0, 0, "", "foo")
    );
    assert_eq!(text.get_type(), "text");

    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        editor.get_editor_state().read(|| {
            get_editor();
        })
    }))
    .expect_err("no active editor");
    let message = panic_message(&*error);
    assert!(
        message.contains("Unable to find an active editor"),
        "{message}"
    );
    // `editorState.read(fn, {editor})`.
    assert!(read_editor_state(Some(&editor), &editor.get_editor_state(), get_editor) == editor);
}

// upstream: lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > toJSON()
#[test]
fn to_json() {
    let editor = mounted_editor();
    editor
        .update(
            || {
                let paragraph = create_paragraph_node();
                let text = create_text_node("Hello world");
                text.select(Some(6), Some(11))?;
                paragraph.append(&[text])?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();
    // Compared as a structure, as upstream does: key order is not part of it.
    assert!(json_to_equal(
        &editor.get_editor_state().to_json(false),
        &parse(r#"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Hello world","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#).unwrap()
    ));
}

// upstream: lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > ensure garbage collection works as expected
#[test]
fn ensure_garbage_collection_works_as_expected() {
    let editor = mounted_editor();
    editor
        .update(
            || {
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("foo")])?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();
    // Removing the first node collects everything under it.
    editor
        .update(
            || get_root().get_first_child().unwrap().remove(false),
            discrete(),
        )
        .unwrap();
    let state = editor.get_editor_state();
    let node_map = state.0.node_map.borrow();
    assert_eq!(node_map.keys().collect::<Vec<_>>(), ["root"]);
    let root = &node_map["root"];
    let (b, e) = (root.base(), root.element().unwrap());
    assert_eq!(
        (
            b.key.as_str(),
            &b.parent,
            &b.prev,
            &b.next,
            &b.slot_host,
            b.slots.is_none()
        ),
        ("root", &None, &None, &None, &None, true)
    );
    assert_eq!((&e.first, &e.last, e.size), (&None, &None, 0));
    assert_eq!(
        (
            e.dir,
            e.format,
            e.indent,
            e.text_format,
            e.style.as_str(),
            e.text_style.as_str()
        ),
        (None, 0, 0.0, 0.0, "", "")
    );
    assert_eq!(root.get_type(), "root");
    let cache = editor.0.root_text_cache.borrow();
    let (cached, cached_text) = cache.as_ref().expect("a cached text");
    assert!(Rc::ptr_eq(cached, root));
    assert_eq!(cached_text, "");
}
