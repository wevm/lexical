//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/LexicalRootHelpers.test.ts`.

#![cfg(feature = "text")]

use crate::text::{
    is_root_text_content_empty, is_root_text_content_empty_curry, root_text_content,
};
use crate::upstream_tests::support::{initialize_unit_test, update};
use crate::{CreateEditorArgs, EditorReadMode, create_paragraph_node, create_text_node, get_root};

// upstream: lexical-utils/src/__tests__/unit/LexicalRootHelpers.test.ts::LexicalRootHelpers tests > textContent
#[test]
fn text_content() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    assert_eq!(
        editor
            .read_with_mode(EditorReadMode::Latest, root_text_content)
            .unwrap(),
        ""
    );
    update(&editor, || {
        let root = get_root();
        let paragraph = create_paragraph_node();
        let text = create_text_node("foo");
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.append(&[text])?;
        assert_eq!(root_text_content(), "foo");
        Ok(())
    });
    assert_eq!(
        editor
            .read_with_mode(EditorReadMode::Latest, root_text_content)
            .unwrap(),
        "foo"
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalRootHelpers.test.ts::LexicalRootHelpers tests > isBlank
#[test]
fn is_blank() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    assert!(
        editor
            .read_with_mode(
                EditorReadMode::Latest,
                is_root_text_content_empty_curry(editor.is_composing(), true)
            )
            .unwrap()
    );
    let composing = editor.is_composing();
    update(&editor, move || {
        let root = get_root();
        let paragraph = create_paragraph_node();
        let text = create_text_node("foo");
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.append(&[text])?;
        assert!(!is_root_text_content_empty(composing, true));
        Ok(())
    });
    assert!(
        !editor
            .read_with_mode(
                EditorReadMode::Latest,
                is_root_text_content_empty_curry(editor.is_composing(), true)
            )
            .unwrap()
    );
}
