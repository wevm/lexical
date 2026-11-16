//! Ports of `packages/lexical-react/src/__tests__/unit/useLexicalIsTextContentEmpty.test.tsx`
//! and `useLexicalIsTextContentEmptyResync.test.tsx`. The hook reads
//! `$isRootTextContentEmptyCurry(editor.isComposing(), trim)` from the
//! editor state when it renders and after every update; these assert the
//! values it reports.

#![cfg(feature = "text")]

use super::super::support::{create_test_editor, set_test_root_element, update};
use crate::text::is_root_text_content_empty_curry;
use crate::{
    CreateEditorArgs, Editor, EditorReadMode, create_paragraph_node, create_text_node, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

/// The value `useLexicalIsTextContentEmpty(editor, trim)` reports.
fn is_text_content_empty(editor: &Editor, trim: bool) -> bool {
    editor
        .read_with_mode(
            EditorReadMode::Latest,
            is_root_text_content_empty_curry(editor.is_composing(), trim),
        )
        .unwrap()
}

/// `makeEditor(text)`: a paragraph holding `text` (none for `''`).
fn make_editor(text: &'static str) -> Editor {
    let editor = create_test_editor(CreateEditorArgs::default());
    update(&editor, move || {
        let paragraph = create_paragraph_node();
        if !text.is_empty() {
            paragraph.append(&[create_text_node(text)])?;
        }
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    });
    editor
}

// upstream: lexical-react/src/__tests__/unit/useLexicalIsTextContentEmpty.test.tsx::useLexicalIsTextContentEmpty > hook works
#[test]
fn hook_works() {
    let editor = create_test_editor(CreateEditorArgs::default());
    set_test_root_element(&editor);
    let reported = Rc::new(RefCell::new(vec![is_text_content_empty(&editor, true)]));
    let (log, listening) = (reported.clone(), editor.clone());
    let unregister = editor.register_update_listener(move |_| {
        log.borrow_mut()
            .push(is_text_content_empty(&listening, true));
    });
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(&[create_text_node("foo")])
    });
    assert_eq!(*reported.borrow(), [true, false]);
    unregister();
}

// upstream: lexical-react/src/__tests__/unit/useLexicalIsTextContentEmptyResync.test.tsx::useLexicalIsTextContentEmpty re-derives its value > follows a change of the trim argument
#[test]
fn follows_a_change_of_the_trim_argument() {
    // Whitespace-only content is empty when trimmed and not empty otherwise,
    // so the answer depends entirely on `trim`.
    let editor = make_editor("   ");
    assert!(!is_text_content_empty(&editor, false));
    assert!(is_text_content_empty(&editor, true));
}

// upstream: lexical-react/src/__tests__/unit/useLexicalIsTextContentEmptyResync.test.tsx::useLexicalIsTextContentEmpty re-derives its value > follows a change of the editor
#[test]
fn follows_a_change_of_the_editor() {
    assert!(!is_text_content_empty(&make_editor("hello"), true));
    assert!(is_text_content_empty(&make_editor(""), true));
}
