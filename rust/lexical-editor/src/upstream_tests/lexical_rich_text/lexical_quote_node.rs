//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts`.
//! Where upstream reads the mounted editor's `outerHTML`, the committed
//! document is rendered as the reconciler does ([`rendered_html`]).

#![cfg(feature = "html")]

use crate::rich_text::create_quote_node;
use crate::upstream_tests::lexical_selection::support::rendered_html;
use crate::upstream_tests::support::{initialize_unit_test, panics, update, update_with};
use crate::{CreateEditorArgs, EditorConfig, create_range_selection, get_root};

const ROOT_OPEN: &str = "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\">";

fn editor_config(theme: &str) -> EditorConfig {
    EditorConfig {
        theme: crate::json::parse(theme)
            .unwrap()
            .as_object()
            .unwrap()
            .clone(),
    }
}

const THEME: &str = r#"{"quote":"my-quote-class"}"#;

// upstream: lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > QuoteNode.constructor
#[test]
fn quote_node_constructor() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let quote_node = create_quote_node(false);
        assert_eq!(quote_node.get_type(), "quote");
        assert_eq!(quote_node.get_text_content(), "");
        Ok(())
    });
    assert!(panics(|| create_quote_node(false)));
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > QuoteNode.createDOM()
#[test]
fn quote_node_create_dom() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let ed = editor.clone();
    update(&editor, move || {
        let quote_node = create_quote_node(false);
        let dom = |theme| {
            quote_node
                .get_latest()
                .create_dom(&quote_node, &editor_config(theme), &ed)
        };
        assert_eq!(
            dom(THEME)?.outer_html(),
            "<blockquote class=\"my-quote-class\"></blockquote>"
        );
        assert_eq!(dom("{}")?.outer_html(), "<blockquote></blockquote>");
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > QuoteNode.updateDOM()
#[test]
fn quote_node_update_dom() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let ed = editor.clone();
    update(&editor, move || {
        let quote_node = create_quote_node(false);
        let dom_element =
            quote_node
                .get_latest()
                .create_dom(&quote_node, &editor_config(THEME), &ed)?;
        assert_eq!(
            dom_element.outer_html(),
            "<blockquote class=\"my-quote-class\"></blockquote>"
        );
        let new_quote_node = create_quote_node(false);
        assert!(
            !new_quote_node
                .get_latest()
                .update_dom(quote_node.get_latest().as_ref())
        );
        assert_eq!(
            dom_element.outer_html(),
            "<blockquote class=\"my-quote-class\"></blockquote>"
        );
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > QuoteNode.insertNewAfter()
#[test]
fn quote_node_insert_new_after() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let quote = update_with(&editor, || {
        let quote_node = create_quote_node(false);
        get_root().append(std::slice::from_ref(&quote_node))?;
        Ok(quote_node)
    });
    assert_eq!(
        format!("{ROOT_OPEN}{}</div>", rendered_html(&editor)),
        format!(
            "{ROOT_OPEN}<blockquote dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></blockquote></div>"
        )
    );
    update(&editor, move || {
        let result = quote
            .insert_new_after(&create_range_selection(), true)?
            .unwrap();
        assert_eq!(result.get_type(), "paragraph");
        assert_eq!(
            result.element_data().get_direction(),
            quote.element_data().get_direction()
        );
        Ok(())
    });
    assert_eq!(
        format!("{ROOT_OPEN}{}</div>", rendered_html(&editor)),
        format!(
            "{ROOT_OPEN}<blockquote dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></blockquote><p dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p></div>"
        )
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > $createQuoteNode()
#[test]
fn create_quote_node_test() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let quote_node = create_quote_node(false);
        let created_quote_node = create_quote_node(false);
        assert_eq!(quote_node.get_type(), created_quote_node.get_type());
        assert_eq!(quote_node.get_parent(), created_quote_node.get_parent());
        assert_ne!(quote_node.get_key(), created_quote_node.get_key());
        Ok(())
    });
}
