//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts`.

#![cfg(feature = "extension")]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::json::Value;
use crate::lexical_utils::is_shadow_root_node;
use crate::rich_text::{create_quote_node, rich_text_extension};
use crate::updates::parse_serialized_node;
use crate::upstream_tests::support::update;
use crate::{
    LexicalExtension, NodeRef, create_paragraph_node, create_text_node, define_extension, get_root,
};

fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("quote-shadow-root-host", ())
    })
    .into()])
    .unwrap()
}

fn paragraph_of(text: &str) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)]).unwrap();
    paragraph
}

/// `QuoteNode.importJSON(json)`.
fn import_json(node: &NodeRef) -> NodeRef {
    parse_serialized_node(&Value::Object(node.export_json(false))).unwrap()
}

// upstream: lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > isShadowRoot() defaults to false and serializes nothing
#[test]
fn is_shadow_root_defaults_to_false_and_serializes_nothing() {
    let editor = build_editor();
    update(&editor, || {
        let quote_node = create_quote_node(false);
        assert!(!is_shadow_root_node(&quote_node));
        assert!(quote_node.export_json(false).get("shadowRoot").is_none());
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > opt-in with $createQuoteNode and setIsShadowRoot
#[test]
fn opt_in_with_create_quote_node_and_set_is_shadow_root() {
    let editor = build_editor();
    update(&editor, || {
        let quote_node = create_quote_node(true);
        assert!(is_shadow_root_node(&quote_node));
        assert_eq!(
            quote_node.export_json(false).get("shadowRoot"),
            Some(&Value::Bool(true))
        );
        quote_node.set_quote_is_shadow_root(false)?;
        assert!(!is_shadow_root_node(&quote_node));
        assert!(quote_node.export_json(false).get("shadowRoot").is_none());
        quote_node.set_quote_is_shadow_root(true)?;
        assert!(is_shadow_root_node(&quote_node));
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > shadow root round-trips through JSON
#[test]
fn shadow_root_round_trips_through_json() {
    let editor = build_editor();
    update(&editor, || {
        let imported = import_json(&create_quote_node(true));
        assert!(is_shadow_root_node(&imported));
        let imported_default = import_json(&create_quote_node(false));
        assert!(!is_shadow_root_node(&imported_default));
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > collapseAtStart() lifts blocks out of a shadow root quote
#[test]
fn collapse_at_start_lifts_blocks_out_of_a_shadow_root_quote() {
    let editor = build_editor();
    update(&editor, || {
        let root = get_root();
        let quote_node = create_quote_node(true);
        quote_node.append(&[paragraph_of("a"), paragraph_of("b")])?;
        root.clear()?;
        root.append(std::slice::from_ref(&quote_node))?;
        quote_node.collapse_at_start(&crate::create_node_selection())?;
        let children = root.get_children();
        assert_eq!(children.len(), 2);
        assert!(children.iter().all(|n| n.get_type() == "paragraph"));
        let texts: Vec<String> = children.iter().map(NodeRef::get_text_content).collect();
        assert_eq!(texts, ["a", "b"]);
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > getTopLevelElement() stops at a shadow root quote
#[test]
fn get_top_level_element_stops_at_a_shadow_root_quote() {
    let editor = build_editor();
    update(&editor, || {
        let root = get_root();
        let quote_node = create_quote_node(true);
        let paragraph = paragraph_of("a");
        quote_node.append(std::slice::from_ref(&paragraph))?;
        root.clear()?;
        root.append(std::slice::from_ref(&quote_node))?;
        assert!(paragraph.get_top_level_element_or_throw()? == paragraph);
        quote_node.set_quote_is_shadow_root(false)?;
        assert!(paragraph.get_top_level_element_or_throw()? == quote_node);
        Ok(())
    });
    editor.dispose();
}
