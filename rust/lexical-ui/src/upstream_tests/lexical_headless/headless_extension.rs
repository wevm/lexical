//! Ports of `packages/lexical-headless/src/__tests__/unit/HeadlessExtension.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::headless::headless_extension;
use crate::upstream_tests::support::TestRootElement;
use crate::{
    CreateEditorArgs, LexicalExtension, RootElementRef, UpdateOptions, create_editor,
    create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `buildHeadlessEditor()`.
fn build_headless_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            headless_extension().into(),
            crate::rich_text::rich_text_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

// upstream: lexical-headless/src/__tests__/unit/HeadlessExtension.test.ts::HeadlessExtension > marks the editor headless and still runs updates and listeners
#[test]
fn marks_the_editor_headless_and_still_runs_updates_and_listeners() {
    let editor = build_headless_editor();
    assert!(editor.is_headless());
    let last = Rc::new(RefCell::new(None::<String>));
    let heard = last.clone();
    let _ = editor.register_update_listener(move |payload| {
        *heard.borrow_mut() = Some(payload.editor_state.read(|| get_root().get_text_content()));
    });
    editor
        .update(
            || {
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("Hello")])?;
                let root = get_root();
                root.clear()?;
                root.append(&[paragraph])?;
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    assert_eq!(last.borrow().as_deref(), Some("Hello"));
    assert_eq!(
        editor.read(|| get_root().get_text_content()).unwrap(),
        "Hello"
    );
}

// upstream: lexical-headless/src/__tests__/unit/HeadlessExtension.test.ts::HeadlessExtension > refuses a root element but allows setting it to null
#[test]
fn refuses_a_root_element_but_allows_setting_it_to_null() {
    let editor = build_headless_editor();
    let error = editor
        .set_root_element(Some(RootElementRef(Rc::new(TestRootElement))))
        .unwrap_err();
    assert!(
        error
            .message()
            .contains("HeadlessExtension: setRootElement is not supported in headless mode"),
        "{error}"
    );
    assert!(editor.get_root_element().is_none());
    assert!(editor.set_root_element(None).is_ok());
}

// upstream: lexical-headless/src/__tests__/unit/HeadlessExtension.test.ts::HeadlessExtension > keeps listener registration working for other extensions
#[test]
fn keeps_listener_registration_working_for_other_extensions() {
    let editor = build_headless_editor();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let heard = calls.clone();
    let unregister = editor.register_root_listener(move |root, prev| {
        heard.borrow_mut().push((root.is_some(), prev.is_some()));
        None
    });
    assert_eq!(*calls.borrow(), [(false, false)]);
    unregister();
}

// upstream: lexical-headless/src/__tests__/unit/HeadlessExtension.test.ts::HeadlessExtension > makes nested editors headless
#[test]
fn makes_nested_editors_headless() {
    let editor = build_headless_editor();
    let nested = create_editor(CreateEditorArgs {
        parent_editor: Some((*editor).clone()),
        ..CreateEditorArgs::default()
    });
    assert!(nested.is_headless());
}
