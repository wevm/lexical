//! Ports of `packages/lexical/src/__tests__/unit/LexicalEditorListener.test.ts`.
//!
//! The `vi.fn()` mocks are call logs; `editor._listeners.<type>.has(listener)`
//! reads the editor's listener list. A root element is a
//! [`TestRootElement`], compared by identity as JS compares the divs.

#![cfg(feature = "extension")]

use super::super::support::TestRootElement;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::{
    LexicalExtension, RootElementRef, Unregister, UpdateOptions, create_paragraph_node,
    create_text_node, define_extension, get_root,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// `buildEditorFromExtensions({name: '@test'})`.
fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension::<(), (), ()>::new(
        "@test",
        (),
    ))
    .into()])
    .unwrap()
}

fn counter(calls: &Rc<Cell<u32>>) -> Unregister {
    let calls = calls.clone();
    Box::new(move || calls.set(calls.get() + 1))
}

fn root() -> RootElementRef {
    RootElementRef(Rc::new(TestRootElement))
}

type RootCall = (Option<RootElementRef>, Option<RootElementRef>);

// upstream: lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerRootListener > can return a function that is called when unregistered
#[test]
fn register_root_listener_can_return_a_function_that_is_called_when_unregistered() {
    let editor = build_editor();
    let callback = Rc::new(Cell::new(0));
    let calls = Rc::new(RefCell::new(Vec::<RootCall>::new()));
    let (c, log) = (callback.clone(), calls.clone());
    let unregister = editor.register_root_listener(move |next, prev| {
        log.borrow_mut().push((next.cloned(), prev.cloned()));
        Some(counter(&c))
    });
    assert_eq!(editor.0.root_listeners.borrow().len(), 1);
    assert_eq!(calls.borrow().len(), 1);
    assert_eq!(calls.borrow().last(), Some(&(None, None)));
    assert_eq!(callback.get(), 0);
    unregister();
    assert_eq!(calls.borrow().len(), 2);
    assert_eq!(calls.borrow().last(), Some(&(None, None)));
    // Called once to unregister the original return value and then again
    // after the finalization (null, …) call
    assert_eq!(callback.get(), 2);
    assert_eq!(editor.0.root_listeners.borrow().len(), 0);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerRootListener > updates the function on each call
#[test]
fn register_root_listener_updates_the_function_on_each_call() {
    let editor = build_editor();
    let callback = Rc::new(Cell::new(0));
    let calls = Rc::new(RefCell::new(Vec::<RootCall>::new()));
    let (c, log) = (callback.clone(), calls.clone());
    let unregister = editor.register_root_listener(move |next, prev| {
        log.borrow_mut().push((next.cloned(), prev.cloned()));
        // `mockImplementationOnce`: only the first call returns it.
        (log.borrow().len() == 1).then(|| counter(&c))
    });
    assert_eq!(calls.borrow().len(), 1);
    assert_eq!(calls.borrow().last(), Some(&(None, None)));
    assert_eq!(callback.get(), 0);
    unregister();
    assert_eq!(calls.borrow().len(), 2);
    assert_eq!(calls.borrow().last(), Some(&(None, None)));
    // Only the first call returns the function
    assert_eq!(callback.get(), 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerRootListener > works when the root element changes too
#[test]
fn register_root_listener_works_when_the_root_element_changes_too() {
    let editor = build_editor();
    let callback_calls = Rc::new(RefCell::new(Vec::<RootCall>::new()));
    let log = callback_calls.clone();
    let initial_root = root();
    let next_root = root();
    editor.set_root_element(Some(initial_root.clone())).unwrap();
    let unregister = editor.register_root_listener(move |next, prev| {
        let (log, next, prev) = (log.clone(), next.cloned(), prev.cloned());
        Some(Box::new(move || log.borrow_mut().push((next, prev))) as Unregister)
    });
    assert!(callback_calls.borrow().is_empty());
    editor.set_root_element(Some(next_root.clone())).unwrap();
    editor.set_root_element(None).unwrap();
    editor.set_root_element(Some(initial_root.clone())).unwrap();
    let (i, n) = (Some(initial_root.clone()), Some(next_root.clone()));
    // We haven't unregistered the call to initialRoot yet
    assert_eq!(
        *callback_calls.borrow(),
        [(i.clone(), None), (n.clone(), i.clone()), (None, n.clone())]
    );
    unregister();
    assert_eq!(
        *callback_calls.borrow(),
        [
            (i.clone(), None),
            (n.clone(), i.clone()),
            (None, n),
            (i.clone(), None),
            (None, i),
        ]
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerEditableListener > can return a function that is called when unregistered
#[test]
fn register_editable_listener_can_return_a_function_that_is_called_when_unregistered() {
    let editor = build_editor();
    let callback = Rc::new(Cell::new(0));
    let listener_calls = Rc::new(Cell::new(0));
    let (c, l) = (callback.clone(), listener_calls.clone());
    let unregister = editor.register_editable_listener(move |_| {
        l.set(l.get() + 1);
        counter(&c)
    });
    assert_eq!(editor.0.editable_listeners.borrow().len(), 1);
    // Not called immediately on registration
    assert_eq!(listener_calls.get(), 0);
    assert_eq!(callback.get(), 0);
    editor.set_editable(false);
    // Called on first change
    assert_eq!(listener_calls.get(), 1);
    assert_eq!(callback.get(), 0);
    unregister();
    // Called on unregister
    assert_eq!(listener_calls.get(), 1);
    assert_eq!(callback.get(), 1);
    assert_eq!(editor.0.editable_listeners.borrow().len(), 0);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerEditableListener > updates the function on each call
#[test]
fn register_editable_listener_updates_the_function_on_each_call() {
    let editor = build_editor();
    let callback = Rc::new(Cell::new(0));
    let calls = Rc::new(RefCell::new(Vec::<bool>::new()));
    let (c, log) = (callback.clone(), calls.clone());
    let unregister = editor.register_editable_listener(move |editable| {
        log.borrow_mut().push(editable);
        (log.borrow().len() == 1).then(|| counter(&c))
    });
    // Not called immediately
    assert!(calls.borrow().is_empty());
    assert_eq!(callback.get(), 0);
    editor.set_editable(false);
    assert_eq!(*calls.borrow(), [false]);
    assert_eq!(callback.get(), 0);
    editor.set_editable(true);
    assert_eq!(*calls.borrow(), [false, true]);
    // Only the first call returns the function
    assert_eq!(callback.get(), 1);
    unregister();
    // Only the first call returns the function
    assert_eq!(callback.get(), 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerEditableListener > works when editable state changes
#[test]
fn register_editable_listener_works_when_editable_state_changes() {
    let editor = build_editor();
    let callback_calls = Rc::new(RefCell::new(Vec::<bool>::new()));
    let listener_calls = Rc::new(Cell::new(0));
    let (log, l) = (callback_calls.clone(), listener_calls.clone());
    let unregister = editor.register_editable_listener(move |editable| {
        l.set(l.get() + 1);
        let log = log.clone();
        Box::new(move || log.borrow_mut().push(editable)) as Unregister
    });
    // Not called on registration
    assert_eq!(listener_calls.get(), 0);
    assert!(callback_calls.borrow().is_empty());
    editor.set_editable(false);
    assert_eq!(listener_calls.get(), 1);
    assert!(callback_calls.borrow().is_empty());
    editor.set_editable(true);
    assert_eq!(listener_calls.get(), 2);
    // Previous callback is called when state changes
    assert_eq!(*callback_calls.borrow(), [false]);
    editor.set_editable(false);
    assert_eq!(*callback_calls.borrow(), [false, true]);
    unregister();
    // Final callback is called on unregister
    assert_eq!(*callback_calls.borrow(), [false, true, false]);
    editor.dispose();
}

// listener return values

#[cfg(feature = "rich-text")]
mod listener_return_values {
    use super::*;

    fn build_rich_text_editor() -> LexicalEditorWithDispose {
        build_editor_from_extensions([define_extension(LexicalExtension {
            dependencies: vec![crate::rich_text::rich_text_extension().into()],
            ..LexicalExtension::<(), (), ()>::new("@test-void-listeners", ())
        })
        .into()])
        .unwrap()
    }

    fn append_paragraph(editor: &LexicalEditorWithDispose, text: &'static str) {
        editor
            .update(
                move || {
                    let root = get_root();
                    root.clear()?;
                    let paragraph = create_paragraph_node();
                    paragraph.append(&[create_text_node(text)])?;
                    root.append(&[paragraph])
                },
                UpdateOptions {
                    discrete: true,
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
    }

    // upstream: lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > listener return values > an update listener may return a non-function value
    #[test]
    fn an_update_listener_may_return_a_non_function_value() {
        let editor = build_rich_text_editor();
        let seen = Rc::new(RefCell::new(Vec::<&str>::new()));
        let log = seen.clone();
        // A listener returning `()`: Rust's counterpart of a value that
        // isn't a function, which is no clean-up.
        let unregister = editor.register_update_listener(move |_| log.borrow_mut().push("x"));
        append_paragraph(&editor, "a");
        append_paragraph(&editor, "b");
        unregister();
        assert_eq!(*seen.borrow(), ["x", "x"]);
        assert_eq!(editor.0.update_listeners.borrow().len(), 0);
        editor.dispose();
    }

    // upstream: lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > listener return values > a text content listener may return a non-function value
    #[test]
    fn a_text_content_listener_may_return_a_non_function_value() {
        let editor = build_rich_text_editor();
        let seen = Rc::new(RefCell::new(Vec::<String>::new()));
        let log = seen.clone();
        let unregister =
            editor.register_text_content_listener(move |text| log.borrow_mut().push(text.into()));
        append_paragraph(&editor, "a");
        append_paragraph(&editor, "b");
        unregister();
        assert_eq!(*seen.borrow(), ["a", "b"]);
        editor.dispose();
    }

    // upstream: lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > listener return values > an update listener returning a function gets it called as cleanup
    #[test]
    fn an_update_listener_returning_a_function_gets_it_called_as_cleanup() {
        let editor = build_rich_text_editor();
        let cleanup = Rc::new(Cell::new(0));
        let c = cleanup.clone();
        let unregister = editor.register_update_listener(move |_| counter(&c));
        append_paragraph(&editor, "a");
        // The cleanup from the first call runs before the second call.
        assert_eq!(cleanup.get(), 0);
        append_paragraph(&editor, "b");
        assert_eq!(cleanup.get(), 1);
        unregister();
        assert_eq!(cleanup.get(), 2);
        editor.dispose();
    }
}
