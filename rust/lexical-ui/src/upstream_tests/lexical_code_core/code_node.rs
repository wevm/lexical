//! Ports of `packages/lexical-code-core/src/__tests__/unit/CodeNode.test.ts`.

#![cfg(feature = "html")]

use super::super::support::{initialize_unit_test, update};
use crate::code::{create_code_node, is_code_node};
use crate::dom::{DomNode, parse_from_string};
use crate::html::generate_nodes_from_dom;
use crate::{CreateEditorArgs, Editor, get_root};
use std::cell::RefCell;
use std::rc::Rc;

/// `initializeUnitTest`'s editor with `{namespace: 'test', nodes:
/// [CodeNode], theme: {code: 'editor-code'}}`.
fn editor() -> Editor {
    initialize_unit_test(CreateEditorArgs {
        namespace: "test".to_owned(),
        nodes: crate::code::code_nodes(),
        theme: crate::json::parse(r#"{"code":"editor-code"}"#)
            .unwrap()
            .as_object()
            .unwrap()
            .clone(),
        ..CreateEditorArgs::default()
    })
}

/// `codeNode.exportDOM(editor).element`, for a code block made by `make`
/// and appended to the root.
fn exported(editor: &Editor, make: fn() -> crate::NodeRef) -> DomNode {
    let out = Rc::new(RefCell::new(None));
    let (slot, e) = (out.clone(), editor.clone());
    update(editor, move || {
        let code_node = make();
        get_root().append(std::slice::from_ref(&code_node))?;
        *slot.borrow_mut() = code_node.get_latest().export_dom(&code_node, &e)?.element;
        Ok(())
    });
    out.take().expect("an exported element")
}

// upstream: lexical-code-core/src/__tests__/unit/CodeNode.test.ts::CodeNode > exports styles through DOM style properties
#[test]
fn exports_styles_through_dom_style_properties() {
    let editor = editor();
    let exported_element = exported(&editor, || {
        let code_node = create_code_node(Some("javascript"), None);
        code_node.set_element_style("padding: 1px; color: blue;");
        code_node
    });
    assert_eq!(
        exported_element.style().get_property_value("padding"),
        "1px"
    );
    assert_eq!(exported_element.style().get_property_value("color"), "blue");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeNode.test.ts::CodeNode > round-trips the theme through exportDOM/importDOM
#[test]
fn round_trips_the_theme_through_export_dom_import_dom() {
    let editor = editor();
    let exported_element = exported(&editor, || {
        create_code_node(Some("javascript"), Some("poimandres"))
    });
    assert_eq!(
        exported_element.get_attribute("data-language").as_deref(),
        Some("javascript")
    );
    assert_eq!(
        exported_element.get_attribute("data-theme").as_deref(),
        Some("poimandres")
    );

    // `document.implementation.createHTMLDocument()`.
    let doc = parse_from_string("<!doctype html><html><head></head><body></body></html>");
    doc.body().unwrap().append_child(&exported_element);

    let e = editor.clone();
    update(&editor, move || {
        let nodes = generate_nodes_from_dom(&e, &doc)?;
        let node = nodes.first().expect("expected a CodeNode");
        assert!(is_code_node(node), "expected a CodeNode");
        assert_eq!(node.get_language().as_deref(), Some("javascript"));
        assert_eq!(node.get_theme().as_deref(), Some("poimandres"));
        Ok(())
    });
}
