//! Ports of `packages/lexical/src/__tests__/unit/DerivedTextFields.test.ts`:
//! the reconciler deriving an element's `textFormat` and `textStyle` from
//! its first text, on an editor with a root element.
#![cfg(all(feature = "extension", feature = "rich-text"))]

use crate::editor::DirtyType;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::updates::get_active_editor;
use crate::{
    InitialEditorStateType, LexicalExtension, RootElement, RootElementRef, UpdateOptions,
    define_extension, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

/// A paragraph whose stored textFormat (0) is not the format of its first
/// text (bold), as a client that never rendered it writes it, and a second
/// paragraph.
const STATE: &str = r#"{"root":{"children":[{"children":[{"detail":0,"format":1,"mode":"normal","style":"color: red","text":"bold","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"plain","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;

struct TestRoot;
impl RootElement for TestRoot {}

/// `$fullReconcile()`.
fn full_reconcile() {
    get_active_editor()
        .expect("an active editor")
        .0
        .dirty
        .borrow_mut()
        .dirty_type = DirtyType::FullReconcile;
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn first_paragraph_text_fields() -> (f64, String) {
    let paragraph = get_root().get_first_child().expect("a paragraph");
    let latest = paragraph.get_latest();
    let element = latest.element().expect("an element");
    (
        element.get_text_format(),
        element.get_text_style().to_owned(),
    )
}

fn paragraph_text_fields() -> Vec<(f64, String)> {
    get_root()
        .get_children()
        .iter()
        .map(|paragraph| {
            let latest = paragraph.get_latest();
            let element = latest.element().expect("an element");
            (
                element.get_text_format(),
                element.get_text_style().to_owned(),
            )
        })
        .collect()
}

/// An editor showing STATE, set before the root element: its first render
/// creates the DOM and derives nothing.
fn setup() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("derived-text-fields", ())
    })
    .into()])
    .unwrap();
    let state = editor.parse_editor_state(STATE).unwrap();
    editor.set_editor_state(&state, None).unwrap();
    editor
        .set_root_element(Some(RootElementRef(Rc::new(TestRoot))))
        .unwrap();
    editor
}

/// Toggles italic on the last paragraph's text and sets its style.
fn edit_last_paragraph() {
    let paragraph = get_root().get_last_child().expect("a paragraph");
    if let Some(text) = paragraph.get_first_child().filter(|t| t.is_text()) {
        text.toggle_format("italic");
        text.set_style("color: blue");
    }
}

// upstream: lexical/src/__tests__/unit/DerivedTextFields.test.ts::derived textFormat and textStyle of an element > a full reconcile changes no element that the update did not change
#[test]
fn a_full_reconcile_changes_no_element_that_the_update_did_not_change() {
    let editor = setup();
    assert_eq!(
        editor.read(first_paragraph_text_fields).unwrap(),
        (0.0, String::new())
    );
    type Calls = Vec<(Vec<String>, Vec<String>)>;
    let calls: Rc<RefCell<Calls>> = Rc::default();
    let recorded = calls.clone();
    let _unregister = editor.register_update_listener(move |payload| {
        recorded.borrow_mut().push((
            payload
                .dirty_elements
                .iter()
                .map(|(k, _)| k.clone())
                .collect(),
            payload.dirty_leaves.clone(),
        ));
    });
    editor
        .update(
            || {
                full_reconcile();
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(
        editor.read(first_paragraph_text_fields).unwrap(),
        (0.0, String::new())
    );
    {
        let calls = calls.borrow();
        assert_eq!(calls.len(), 1);
        // Only the root, which a full reconcile marks.
        assert_eq!(calls[0], (vec!["root".to_owned()], Vec::<String>::new()));
    }
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/DerivedTextFields.test.ts::derived textFormat and textStyle of an element > a full reconcile with an edit derives them for the element it changes only
#[test]
fn a_full_reconcile_with_an_edit_derives_them_for_the_element_it_changes_only() {
    let editor = setup();
    editor
        .update(
            || {
                full_reconcile();
                edit_last_paragraph();
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(
        editor.read(paragraph_text_fields).unwrap(),
        vec![(0.0, String::new()), (2.0, "color: blue".to_owned())]
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/DerivedTextFields.test.ts::derived textFormat and textStyle of an element > an element whose first text changes takes its format and style
#[test]
fn an_element_whose_first_text_changes_takes_its_format_and_style() {
    let editor = setup();
    editor
        .update(
            || {
                edit_last_paragraph();
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(
        editor
            .read(|| {
                let paragraph = get_root().get_last_child().unwrap();
                let latest = paragraph.get_latest();
                let element = latest.element().unwrap();
                (
                    element.get_text_format(),
                    element.get_text_style().to_owned(),
                )
            })
            .unwrap(),
        (2.0, "color: blue".to_owned())
    );
    editor.dispose();
}
