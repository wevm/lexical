//! Ports of `packages/lexical-html/src/__tests__/unit/LexicalHtmlBackwardCompat.test.ts`.

use super::super::support::update;
use crate::headless::create_headless_editor;
use crate::html::generate_html_from_nodes;
use crate::{CreateEditorArgs, Editor, create_paragraph_node, create_text_node, get_root};

fn editor_with(text: &'static str) -> Editor {
    let editor = create_headless_editor(CreateEditorArgs::default());
    update(&editor, move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node(text)])?;
        get_root().append(&[paragraph])
    });
    editor
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtmlBackwardCompat.test.ts::$generateHtmlFromNodes backward compatibility > works inside legacy editor.getEditorState().read(cb) scope (no active editor)
#[test]
fn works_inside_legacy_editor_state_read_scope() {
    let editor = editor_with("Hello world");
    let html = editor
        .get_editor_state()
        .read(|| generate_html_from_nodes(&editor, None))
        .unwrap();
    assert!(html.contains("Hello world"), "{html}");
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtmlBackwardCompat.test.ts::$generateHtmlFromNodes backward compatibility > still works inside editor.read() scope (active editor present)
#[test]
fn still_works_inside_editor_read_scope() {
    let editor = editor_with("Test content");
    let html = editor
        .read(|| generate_html_from_nodes(&editor, None))
        .unwrap()
        .unwrap();
    assert!(html.contains("Test content"), "{html}");
}
