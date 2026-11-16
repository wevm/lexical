//! Ports of `packages/lexical-code-prism/src/__tests__/unit/CodePrismRetainSelection.test.ts`.
//!
//! `CodePrismExtension` highlights with Prism; here with [`TestTokenizer`]
//! (see `support`), which splits `const x = 1;` where Prism's JavaScript
//! grammar does.

#![cfg(all(feature = "extension", feature = "rich-text", feature = "markdown"))]

use super::super::support::update;
use super::support::{TestTokenizer, code_prism_extension};
use crate::code::{create_code_node, is_code_node};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::markdown::{convert_from_markdown_string, transformers};
use crate::{
    Editor, LexicalExtension, create_paragraph_node, create_text_node, define_extension, get_root,
    get_selection,
};

fn create_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            code_prism_extension(TestTokenizer::default()),
        ],
        ..LexicalExtension::<(), (), ()>::new("code-prism-retain-selection-test", ())
    })
    .into()])
    .unwrap()
}

/// `$describeCaret()`: `(blocks, inCodeBlock, offset, text)`.
fn describe_caret(editor: &Editor) -> (Vec<String>, bool, usize, String) {
    editor
        .read(|| {
            let selection = get_selection()
                .and_then(|s| s.range())
                .expect("a range selection");
            let anchor_node = selection.anchor.get_node().unwrap();
            (
                get_root()
                    .get_children()
                    .iter()
                    .map(|c| c.get_type().to_owned())
                    .collect(),
                is_code_node(&anchor_node) || anchor_node.get_parents().iter().any(is_code_node),
                selection.anchor.offset,
                anchor_node.get_text_content(),
            )
        })
        .unwrap()
}

/// `$seedSelection()`.
fn seed_selection(editor: &Editor) {
    update(editor, || {
        let paragraph = create_paragraph_node();
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?;
        Ok(())
    });
}

fn import_markdown(editor: &Editor, markdown: &'static str) {
    update(editor, move || {
        convert_from_markdown_string(markdown, &transformers(), None, false, false)
    });
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

// upstream: lexical-code-prism/src/__tests__/unit/CodePrismRetainSelection.test.ts::Prism highlighting only retains a selection it owns (#6305) > importing markdown that ends in a code block leaves the caret at the document start
#[test]
fn importing_markdown_that_ends_in_a_code_block_leaves_the_caret_at_the_start() {
    let editor = create_editor();
    seed_selection(&editor);
    import_markdown(&editor, "hello world\n\n```\necho hello world\n```");
    assert_eq!(
        describe_caret(&editor),
        (
            strings(&["paragraph", "code"]),
            false,
            0,
            "hello world".to_owned()
        )
    );
}

// upstream: lexical-code-prism/src/__tests__/unit/CodePrismRetainSelection.test.ts::Prism highlighting only retains a selection it owns (#6305) > the caret is not dragged into the last of several imported code blocks
#[test]
fn the_caret_is_not_dragged_into_the_last_of_several_imported_code_blocks() {
    let editor = create_editor();
    seed_selection(&editor);
    import_markdown(
        &editor,
        "hello world\n\n```\nfirst\n```\n\nmiddle\n\n```\nlast\n```",
    );
    assert_eq!(
        describe_caret(&editor),
        (
            strings(&["paragraph", "code", "paragraph", "code"]),
            false,
            0,
            "hello world".to_owned()
        )
    );
}

// upstream: lexical-code-prism/src/__tests__/unit/CodePrismRetainSelection.test.ts::Prism highlighting only retains a selection it owns (#6305) > a caret inside the code block is still retained across highlighting
#[test]
fn a_caret_inside_the_code_block_is_still_retained_across_highlighting() {
    let editor = create_editor();
    update(&editor, || {
        let code_node = create_code_node(Some("javascript"), None);
        let text_node = create_text_node("const x = 1;");
        code_node.append(std::slice::from_ref(&text_node))?;
        let root = get_root();
        root.clear()?;
        root.append(&[code_node])?;
        text_node.select(Some(6), Some(6))?;
        Ok(())
    });
    let (_, in_code_block, offset, text) = describe_caret(&editor);
    assert!(in_code_block);
    assert_eq!(text, " x ");
    assert_eq!(offset, 1);
}
