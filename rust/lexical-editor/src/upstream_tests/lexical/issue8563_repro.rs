//! Ports of `packages/lexical/src/__tests__/unit/Issue8563Repro.test.ts`.
//!
//! The root element is a [`TestRootElement`], which makes the reconciler
//! run; where upstream reads the root element's `textContent`, the port
//! reads the committed tree, as the other ports compare a mounted editor's
//! DOM.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::{set_test_root_element, shape};
use crate::extension::build_editor_from_extensions;
use crate::{
    LexicalExtension, UpdateOptions, create_paragraph_node, create_text_node, define_extension,
    get_root,
};

// upstream: lexical/src/__tests__/unit/Issue8563Repro.test.ts::Issue #8563: full reconcile with same-size child key swap > undo (setEditorState) does not crash when a child is replaced by a different-key child of equal count
#[test]
fn undo_set_editor_state_does_not_crash_when_a_child_is_replaced_by_a_different_key_child_of_equal_count()
 {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("issue-8563-repro", ())
    })
    .into()])
    .unwrap();
    // `onError` collecting errors: each call's `Result` must be `Ok`.
    set_test_root_element(&editor);
    let discrete = || UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    };
    editor
        .update(
            || {
                let root = get_root();
                root.clear()?;
                for i in 0..5 {
                    let paragraph = create_paragraph_node();
                    paragraph.append(&[create_text_node(&format!("line {i}"))])?;
                    root.append(&[paragraph])?;
                }
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let state_a = editor.get_editor_state();
    editor
        .update(
            || {
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("replaced")])?;
                get_root()
                    .get_last_child_or_throw()?
                    .replace(&paragraph, false)?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor.set_editor_state(&state_a, None).unwrap();
    assert_eq!(
        editor.read(|| get_root().get_text_content()).unwrap(),
        "line 0\n\nline 1\n\nline 2\n\nline 3\n\nline 4"
    );
    assert_eq!(
        shape(&editor),
        "<paragraph>[line 0]</paragraph><paragraph>[line 1]</paragraph><paragraph>[line 2]</paragraph><paragraph>[line 3]</paragraph><paragraph>[line 4]</paragraph>"
    );
    editor.dispose();
}
