//! Ports of `packages/lexical-code-prism/src/__tests__/unit/CodePrismUnknownLanguage.test.ts`.
//!
//! `CodePrismExtension` with Prism; here with [`TestTokenizer`] (see
//! `support`), to which `After` is as unknown as to Prism.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::update;
use super::support::{TestTokenizer, code_prism_extension};
use crate::code::{create_code_node, is_code_highlight_node, is_code_node};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::json::{Value, parse};
use crate::{
    KEY_TAB_COMMAND, KeyboardEvent, LexicalExtension, NodeRef, PointType, Which, create_text_node,
    define_extension, get_root,
};
use std::rc::Rc;

fn create_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            code_prism_extension(TestTokenizer::default()),
        ],
        ..LexicalExtension::<(), (), ()>::new("prism-unknown-language", ())
    })
    .into()])
    .unwrap()
}

/// `$getCodeNode()`.
fn get_code_node() -> NodeRef {
    let code_node = get_root().get_first_child().expect("Expected a CodeNode");
    assert!(is_code_node(&code_node), "Expected a CodeNode");
    code_node
}

/// `toMatchObject`: every key of `expected` is in `actual` with that value.
fn matches_object(actual: &crate::json::Object, expected: &Value) -> bool {
    expected
        .as_object()
        .unwrap()
        .iter()
        .all(|(k, v)| actual.get(k) == Some(v))
}

fn set_code(editor: &crate::Editor, text: &'static str) {
    update(editor, move || {
        let root = get_root();
        root.clear()?;
        let code = create_code_node(Some("After"), None);
        code.append(&[create_text_node(text)])?;
        root.append(&[code])
    });
}

// upstream: lexical-code-prism/src/__tests__/unit/CodePrismUnknownLanguage.test.ts::Prism with a language it does not know > splits the text into CodeHighlightNode + LineBreakNode + TabNode
#[test]
fn splits_the_text_into_code_highlight_line_break_and_tab_nodes() {
    let editor = create_editor();
    set_code(&editor, "a\n\tb");
    editor
        .read(|| {
            let code_node = get_code_node();
            assert_eq!(code_node.get_language().as_deref(), Some("After"));
            assert!(!code_node.get_is_syntax_highlight_supported());
            let children = code_node.get_children();
            let expected = [
                r#"{"text":"a","type":"code-highlight"}"#,
                r#"{"type":"linebreak"}"#,
                r#"{"type":"tab"}"#,
                r#"{"text":"b","type":"code-highlight"}"#,
            ];
            assert_eq!(children.len(), expected.len());
            for (child, expected) in children.iter().zip(expected) {
                assert!(
                    matches_object(&child.export_json(false), &parse(expected).unwrap()),
                    "{expected}"
                );
            }
            assert_eq!(
                children
                    .iter()
                    .filter(|c| is_code_highlight_node(c))
                    .map(NodeRef::get_highlight_type)
                    .collect::<Vec<_>>(),
                [None, None]
            );
        })
        .unwrap();
}

// upstream: lexical-code-prism/src/__tests__/unit/CodePrismUnknownLanguage.test.ts::Prism with a language it does not know > Tab over the whole block indents every line
#[test]
fn tab_over_the_whole_block_indents_every_line() {
    let editor = create_editor();
    set_code(&editor, "one\ntwo");
    // Select all, as the browser reports it: text point to text point.
    update(&editor, || {
        let code_node = get_code_node();
        let first = code_node.get_first_descendant().unwrap();
        let last = code_node.get_last_descendant().unwrap();
        assert!(first.is_text() && last.is_text());
        first.select(Some(0), Some(0))?.point_set(
            Which::Focus,
            last.get_key(),
            last.get_text_content_size(),
            PointType::Text,
        )
    });
    editor
        .dispatch_command(
            &KEY_TAB_COMMAND,
            Rc::new(KeyboardEvent {
                key: "Tab".to_owned(),
                ..KeyboardEvent::default()
            }),
        )
        .unwrap();
    assert_eq!(
        editor.read(|| get_code_node().get_text_content()).unwrap(),
        "\tone\n\ttwo"
    );
}
