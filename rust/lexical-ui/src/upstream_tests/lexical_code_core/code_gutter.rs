//! Ports of `packages/lexical-code-core/src/__tests__/unit/CodeGutter.test.ts`.
//! JS reads the `data-gutter` attribute `registerCodeGutter` writes on the
//! code block's element; the port reads the last value it gave the app for
//! the block.

use super::super::support::{initialize_unit_test, update};
use crate::code::{create_code_highlight_node, create_code_node, register_code_gutter};
use crate::nodes::line_break::create_line_break_node;
use crate::{CreateEditorArgs, Editor, NodeKey, NodeRef, get_node_by_key, get_root};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Gutters = Rc<RefCell<HashMap<String, String>>>;

/// `initializeUnitTest`'s editor with `CodeNode`, and `registerCodeGutter`
/// on it, recording each block's gutter as JS's attribute holds it.
fn editor() -> (Editor, Gutters) {
    let editor = initialize_unit_test(CreateEditorArgs {
        namespace: "test".to_owned(),
        nodes: crate::code::code_nodes(),
        ..CreateEditorArgs::default()
    });
    let gutters = Gutters::default();
    let g = gutters.clone();
    std::mem::forget(
        register_code_gutter(&editor, move |key, gutter| {
            g.borrow_mut().insert(key.to_owned(), gutter.to_owned());
        })
        .unwrap(),
    );
    (editor, gutters)
}

/// A code block of `children`, appended to the root; its key.
fn append_code(editor: &Editor, children: fn() -> Vec<NodeRef>) -> NodeKey {
    let key = Rc::new(RefCell::new(String::new()));
    let k = key.clone();
    update(editor, move || {
        let code_node = create_code_node(Some("javascript"), None);
        code_node.append(&children())?;
        get_root().append(std::slice::from_ref(&code_node))?;
        *k.borrow_mut() = code_node.get_key().to_owned();
        Ok(())
    });
    key.take()
}

fn gutter(gutters: &Gutters, key: &str) -> Option<String> {
    gutters.borrow().get(key).cloned()
}

fn text(s: &str) -> NodeRef {
    create_code_highlight_node(s, None)
}

// upstream: lexical-code-core/src/__tests__/unit/CodeGutter.test.ts::registerCodeGutter > numbers a single line
#[test]
fn numbers_a_single_line() {
    let (editor, gutters) = editor();
    let key = append_code(&editor, || vec![text("const a = 1;")]);
    assert_eq!(gutter(&gutters, &key).as_deref(), Some("1"));
}

// upstream: lexical-code-core/src/__tests__/unit/CodeGutter.test.ts::registerCodeGutter > numbers several lines
#[test]
fn numbers_several_lines() {
    let (editor, gutters) = editor();
    let key = append_code(&editor, || {
        vec![
            text("a"),
            create_line_break_node(),
            text("b"),
            create_line_break_node(),
            text("c"),
        ]
    });
    assert_eq!(gutter(&gutters, &key).as_deref(), Some("1\n2\n3"));
}

// upstream: lexical-code-core/src/__tests__/unit/CodeGutter.test.ts::registerCodeGutter > numbers an empty trailing line
#[test]
fn numbers_an_empty_trailing_line() {
    let (editor, gutters) = editor();
    let key = append_code(&editor, || vec![text("a"), create_line_break_node()]);
    assert_eq!(gutter(&gutters, &key).as_deref(), Some("1\n2"));
}

// upstream: lexical-code-core/src/__tests__/unit/CodeGutter.test.ts::registerCodeGutter > follows an edit that changes the line count
#[test]
fn follows_an_edit_that_changes_the_line_count() {
    let (editor, gutters) = editor();
    let key = append_code(&editor, || {
        vec![text("a"), create_line_break_node(), text("b")]
    });
    assert_eq!(gutter(&gutters, &key).as_deref(), Some("1\n2"));

    let k = key.clone();
    update(&editor, move || {
        let code_node = get_node_by_key(&k).expect("a CodeNode");
        code_node.append(&[create_line_break_node(), text("c")])
    });
    assert_eq!(gutter(&gutters, &key).as_deref(), Some("1\n2\n3"));

    let k = key.clone();
    update(&editor, move || {
        let code_node = get_node_by_key(&k).expect("a CodeNode");
        let size = code_node.get_children_size();
        code_node.splice(1, size - 1, &[])
    });
    assert_eq!(gutter(&gutters, &key).as_deref(), Some("1"));
}

/// Not upstream: the attribute is rewritten only when the number of
/// children changes (JS's `__cachedChildrenLength`), so an edit of a
/// line's text gives the app nothing new; and a headless editor registers
/// nothing.
#[test]
fn only_a_changed_child_count_rewrites_the_gutter() {
    let editor = initialize_unit_test(CreateEditorArgs {
        nodes: crate::code::code_nodes(),
        ..CreateEditorArgs::default()
    });
    let calls = Rc::new(RefCell::new(Vec::<String>::new()));
    let c = calls.clone();
    let _unregister = register_code_gutter(&editor, move |_, gutter| {
        c.borrow_mut().push(gutter.to_owned());
    })
    .unwrap();
    let key = append_code(&editor, || vec![text("a")]);
    let k = key.clone();
    update(&editor, move || {
        let code_node = get_node_by_key(&k).expect("a CodeNode");
        let first = code_node.get_first_child().expect("its text");
        first.replace(&text("changed"), false)?;
        Ok(())
    });
    assert_eq!(*calls.borrow(), ["1"]);

    let headless = crate::create_headless_editor(CreateEditorArgs {
        nodes: crate::code::code_nodes(),
        ..CreateEditorArgs::default()
    });
    let called = Rc::new(RefCell::new(false));
    let c = called.clone();
    register_code_gutter(&headless, move |_, _| *c.borrow_mut() = true).unwrap()();
    assert!(!*called.borrow());
}
