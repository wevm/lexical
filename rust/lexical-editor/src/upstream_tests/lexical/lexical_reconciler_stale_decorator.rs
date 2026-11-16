//! Ports of `packages/lexical/src/__tests__/unit/LexicalReconcilerStaleDecorator.test.ts`.

#![cfg(feature = "extension")]

use crate::extension::build_editor_from_extensions;
use crate::upstream_tests::support::{TestDecoratorNode, create_test_decorator_node};
use crate::{
    InitialEditorStateType, LexicalError, LexicalExtension, NodeClass, UpdateOptions,
    create_paragraph_node, define_extension, get_node_by_key, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

// upstream: lexical/src/__tests__/unit/LexicalReconcilerStaleDecorator.test.ts::LexicalReconciler — last-child decorator removal > does not throw when an inline DecoratorNode is removed as the last child of an element
#[test]
fn does_not_throw_when_an_inline_decorator_node_is_removed_as_the_last_child_of_an_element() {
    let decorator_key = Rc::new(RefCell::new(String::new()));
    let key = decorator_key.clone();
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
            let paragraph = create_paragraph_node();
            let decorator = create_test_decorator_node();
            paragraph.append(std::slice::from_ref(&decorator))?;
            get_root().append(&[paragraph])?;
            *key.borrow_mut() = decorator.get_key().to_owned();
            Ok(())
        }))),
        nodes: vec![NodeClass::of::<TestDecoratorNode>()],
        ..LexicalExtension::<(), (), ()>::new("lexical-reconciler-stale-decorator", ())
    })
    .into()])
    .unwrap();
    let key = decorator_key.borrow().clone();
    let result = editor.update(
        move || {
            let decorator = get_node_by_key(&key).ok_or_else(|| {
                LexicalError::new("decorator missing — initial state did not commit")
            })?;
            decorator.remove(false)
        },
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        },
    );
    assert!(result.is_ok(), "{result:?}");
}
