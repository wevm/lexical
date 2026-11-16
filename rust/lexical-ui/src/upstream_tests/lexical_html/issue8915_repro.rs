//! Ports of `packages/lexical-html/src/__tests__/unit/Issue8915Repro.test.ts`.
//! Each `test.each` over the capitalization formats is one test.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::update;
use crate::dom::{DomNode, parse_from_string};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::generate_nodes_from_dom_via_extension;
use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
use crate::{
    Editor, InitialEditorStateType, LexicalExtension, NodeRef, create_paragraph_node,
    create_text_node, define_extension, get_root,
};

const CAPITALIZATION_FORMATS: [&str; 3] = ["lowercase", "uppercase", "capitalize"];

fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[issue-8915]", ())
    })
    .into()])
    .unwrap()
}

/// `$seed(format)`.
fn seed(editor: &Editor, format: &'static str) {
    update(editor, move || {
        let text = create_text_node("Hello");
        text.toggle_format(format);
        let paragraph = create_paragraph_node();
        paragraph.append(&[text])?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    });
}

fn export_html(editor: &Editor) -> String {
    editor
        .read(|| generate_html_from_nodes(editor, None))
        .unwrap()
        .unwrap()
}

fn parse(html: &str) -> DomNode {
    parse_from_string(&format!("<!doctype html><html><body>{html}</body></html>"))
}

/// `firstTextNode(nodes)`: depth first.
fn first_text_node(nodes: &[NodeRef]) -> NodeRef {
    let mut stack: Vec<NodeRef> = nodes.to_vec();
    while !stack.is_empty() {
        let node = stack.remove(0);
        if node.is_text() {
            return node;
        }
        if node.is_element() {
            let children = node.get_children();
            stack.splice(0..0, children);
        }
    }
    panic!("expected a TextNode");
}

// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > exportDOM writes text-transform for lowercase
// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > exportDOM writes text-transform for uppercase
// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > exportDOM writes text-transform for capitalize
#[test]
fn export_dom_writes_text_transform() {
    for format in CAPITALIZATION_FORMATS {
        let editor = build_editor();
        seed(&editor, format);
        let html = export_html(&editor);
        assert!(
            html.contains(&format!("text-transform: {format}")),
            "{html}"
        );
    }
}

// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOM restores lowercase
// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOM restores uppercase
// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOM restores capitalize
#[test]
fn generate_nodes_from_dom_restores_text_transform() {
    for format in CAPITALIZATION_FORMATS {
        let editor = build_editor();
        seed(&editor, format);
        let html = export_html(&editor);
        let e = (*editor).clone();
        update(&editor, move || {
            let text_node = first_text_node(&generate_nodes_from_dom(&e, &parse(&html))?);
            assert!(text_node.has_format(format), "{format}");
            Ok(())
        });
    }
}

// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOMViaExtension restores lowercase
// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOMViaExtension restores uppercase
// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOMViaExtension restores capitalize
#[test]
fn generate_nodes_from_dom_via_extension_restores_text_transform() {
    for format in CAPITALIZATION_FORMATS {
        let editor = build_editor();
        seed(&editor, format);
        let html = export_html(&editor);
        update(&editor, move || {
            let text_node =
                first_text_node(&generate_nodes_from_dom_via_extension(&parse(&html), None)?);
            assert!(text_node.has_format(format), "{format}");
            Ok(())
        });
    }
}

// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > text-transform: none clears an inherited capitalization format
#[test]
fn text_transform_none_clears_an_inherited_capitalization_format() {
    let editor = build_editor();
    update(&editor, || {
        let text_node = first_text_node(&generate_nodes_from_dom_via_extension(
            &parse(
                r#"<p><span style="text-transform: uppercase"><span style="text-transform: none">Hello</span></span></p>"#,
            ),
            None,
        )?);
        assert!(!text_node.has_format("uppercase"));
        Ok(())
    });
}

// upstream: lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > an unrelated text-transform: none does not clear other formats
#[test]
fn an_unrelated_text_transform_none_does_not_clear_other_formats() {
    let editor = build_editor();
    let e = (*editor).clone();
    update(&editor, move || {
        let nodes = generate_nodes_from_dom(
            &e,
            &parse(r#"<p><span style="font-weight: bold; text-transform: none;">Hello</span></p>"#),
        )?;
        let text_node = first_text_node(&nodes);
        assert!(text_node.has_format("bold"));
        assert!(!text_node.has_format("uppercase"));
        Ok(())
    });
}
