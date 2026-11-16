//! Ports of `packages/lexical-code-prism/src/__tests__/unit/CodePrismNullDefaultLanguage.test.ts`.
//!
//! `{...PrismTokenizer, defaultLanguage: null}` is a [`TestTokenizer`]
//! (see `support`) without a default language.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::update_with;
use super::support::{TestTokenizer, code_prism_extension};
use crate::code::create_code_node;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::{LexicalExtension, NodeRef, create_text_node, define_extension, get_root};

fn create_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            code_prism_extension(TestTokenizer {
                default_language: None,
            }),
        ],
        ..LexicalExtension::<(), (), ()>::new("prism-default-null", ())
    })
    .into()])
    .unwrap()
}

fn append_code(editor: &crate::Editor, text: &'static str) -> NodeRef {
    update_with(editor, move || {
        let code_node = create_code_node(None, None);
        code_node.append(&[create_text_node(text)])?;
        get_root().append(std::slice::from_ref(&code_node))?;
        Ok(code_node)
    })
}

// upstream: lexical-code-prism/src/__tests__/unit/CodePrismNullDefaultLanguage.test.ts::Prism defaultLanguage: null (#7235) > leaves `__language` unset and skips highlight mutation
#[test]
fn leaves_language_unset_and_skips_highlight_mutation() {
    let editor = create_editor();
    let code_node = append_code(&editor, "hello");
    editor
        .read(|| assert_eq!(code_node.get_language(), None))
        .unwrap();
}

// upstream: lexical-code-prism/src/__tests__/unit/CodePrismNullDefaultLanguage.test.ts::Prism defaultLanguage: null (#7235) > splits text into CodeHighlightNode + LineBreakNode + TabNode for `\n` / `\t` so indent + line-move handlers stay compatible
#[test]
fn splits_text_into_code_highlight_line_break_and_tab_nodes() {
    let editor = create_editor();
    let code_node = append_code(&editor, "a\n\tb");
    editor
        .read(|| {
            assert_eq!(
                code_node
                    .get_children()
                    .iter()
                    .map(|c| c.get_type())
                    .collect::<Vec<_>>(),
                ["code-highlight", "linebreak", "tab", "code-highlight"]
            );
        })
        .unwrap();
}
